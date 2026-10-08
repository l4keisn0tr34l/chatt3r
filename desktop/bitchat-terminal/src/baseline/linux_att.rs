//! Linux-only LE ATT path for dual-mode phones whose BlueZ Device1.Connect
//! chooses classic Bluetooth profiles. No pairing, adapter reset, or message
//! replay. ATT and GATT parsing live here; packet validation stays upstream.
use std::io;
use std::mem::size_of;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::unix::AsyncFd;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{timeout, Duration};
use uuid::Uuid;

const ATT_CID: u16 = 4;
const LE_PUBLIC: u8 = 1;
const NOTIFY: u8 = 0x1b;
const MAX_ATT: usize = 517;
pub const TEXT_ATT_MTU: u16 = 185;
pub const FILE_ATT_MTU: u16 = 517; // opt-in attempt to fit observed 504-byte file values (+3 ATT)

#[repr(C)]
struct L2Addr {
    family: libc::sa_family_t,
    psm: u16,
    address: [u8; 6],
    cid: u16,
    address_type: u8,
}

fn invalid(text: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, text)
}

#[derive(Debug)]
struct SocketSetupError {
    stage: &'static str,
    source: io::Error,
}

impl std::fmt::Display for SocketSetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LE ATT {}: {}", self.stage, self.source)
    }
}

impl std::error::Error for SocketSetupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

pub(super) fn socket_setup_error(stage: &'static str, source: io::Error) -> io::Error {
    io::Error::new(source.kind(), SocketSetupError { stage, source })
}

pub fn setup_errno(error: &io::Error) -> Option<i32> {
    error.raw_os_error().or_else(|| {
        error
            .get_ref()?
            .downcast_ref::<SocketSetupError>()?
            .source
            .raw_os_error()
    })
}

fn le16(bytes: &[u8]) -> io::Result<u16> {
    if bytes.len() < 2 {
        return Err(invalid("short ATT handle"));
    }
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn negotiated_att_mtu(requested: u16, reply: &[u8]) -> io::Result<u16> {
    if reply.len() != 3 || reply[0] != 0x03 {
        return Err(invalid("malformed ATT MTU response"));
    }
    let peer_mtu = le16(&reply[1..])?;
    if !(23..=MAX_ATT as u16).contains(&peer_mtu) {
        return Err(invalid("invalid peer ATT MTU"));
    }
    Ok(peer_mtu.min(requested))
}

fn matches_uuid(bytes: &[u8], uuid: Uuid) -> bool {
    bytes.len() == 16
        && bytes
            .iter()
            .rev()
            .copied()
            .eq(uuid.as_bytes().iter().copied())
}

/// iOS is dual-role: it can write to us while we discover its GATT server.
/// We expose no server attributes, so reject incoming requests separately.
fn incoming_att_request(packet: &[u8], local_mtu: u16) -> Option<Vec<u8>> {
    let opcode = *packet.first()?;
    if opcode == 0x02 && packet.len() == 3 {
        let mut reply = vec![0x03];
        reply.extend(local_mtu.to_le_bytes());
        return Some(reply);
    }
    if matches!(
        opcode,
        0x04 | 0x06 | 0x08 | 0x0a | 0x0c | 0x0e | 0x10 | 0x12 | 0x16 | 0x18
    ) && packet.len() >= 3
    {
        return Some(vec![0x01, opcode, packet[1], packet[2], 0x01]); // Invalid Handle
    }
    None
}

fn att_error(request: u8, response: &[u8]) -> Option<io::Error> {
    if response.first() != Some(&0x01) {
        return None;
    }
    if response.len() != 5 || response[1] != request {
        return Some(invalid("malformed ATT error response"));
    }
    if response[4] == 0x0a {
        let description = match request {
            0x10 => "BitChat LE service not found; open BitChat on the phone",
            0x08 => "BitChat LE characteristic not found",
            0x04 => "BitChat LE notify descriptor not found",
            _ => "ATT attribute not found",
        };
        return Some(io::Error::new(io::ErrorKind::NotFound, description));
    }
    Some(io::Error::other(format!(
        "ATT error for opcode 0x{request:02x}: {}",
        hex::encode(response)
    )))
}

/// A single ATT request is in flight; a dedicated reader routes notifications
/// separately so an announcement cannot be mistaken for a request response.
pub struct DirectAtt {
    socket: Arc<AsyncFd<OwnedFd>>,
    replies: Mutex<mpsc::Receiver<Vec<u8>>>,
    notifications: Mutex<Option<mpsc::Receiver<Vec<u8>>>>,
    connected: Arc<AtomicBool>,
    read_task: tokio::task::JoinHandle<()>,
    characteristic: u16,
    mtu: u16,
}

impl DirectAtt {
    pub async fn connect(
        address: [u8; 6],
        service: Uuid,
        characteristic: Uuid,
        requested_mtu: u16,
    ) -> io::Result<Arc<Self>> {
        if !matches!(requested_mtu, TEXT_ATT_MTU | FILE_ATT_MTU) {
            return Err(invalid("unsupported direct LE ATT MTU request"));
        }
        let socket = Arc::new(
            timeout(Duration::from_secs(20), connect_socket(address))
                .await
                .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "LE connect timed out"))??,
        );
        let (reply_tx, reply_rx) = mpsc::channel(8);
        let (notify_tx, notify_rx) = mpsc::channel(64);
        let connected = Arc::new(AtomicBool::new(true));
        let alive = connected.clone();
        let reader = socket.clone();
        let value_handle = Arc::new(std::sync::atomic::AtomicU16::new(0));
        let chosen = value_handle.clone();
        let read_task = tokio::spawn(async move {
            loop {
                let packet = match read_packet(&reader).await {
                    Ok(packet) => packet,
                    Err(_) => break,
                };
                if matches!(packet.first(), Some(&NOTIFY | &0x1d)) && packet.len() >= 3 {
                    if le16(&packet[1..3]).ok() == Some(chosen.load(Ordering::Relaxed))
                        && notify_tx.try_send(packet[3..].to_vec()).is_err()
                    {
                        break;
                    }
                    if packet[0] == 0x1d && write_packet(&reader, &[0x1e]).await.is_err() {
                        break;
                    }
                } else if let Some(response) = incoming_att_request(&packet, requested_mtu) {
                    if write_packet(&reader, &response).await.is_err() {
                        break;
                    }
                } else if reply_tx.send(packet).await.is_err() {
                    break;
                }
            }
            alive.store(false, Ordering::Release);
        });
        let mut link = Self {
            socket,
            replies: Mutex::new(reply_rx),
            notifications: Mutex::new(Some(notify_rx)),
            connected,
            read_task,
            characteristic: 0,
            mtu: 23,
        };
        let mut exchange = vec![0x02];
        exchange.extend(requested_mtu.to_le_bytes());
        let mtu_reply = link.request(&exchange, 0x03).await?;
        link.mtu = negotiated_att_mtu(requested_mtu, &mtu_reply)?;
        if link.mtu < 39 {
            return Err(invalid("LE ATT MTU too small for signed BitChat fragments"));
        }
        let (start, end) = link.find_service(service).await?;
        let (value, properties) = link.find_characteristic(start, end, characteristic).await?;
        if properties & 0x10 == 0 || properties & 0x08 == 0 {
            return Err(invalid(
                "BitChat characteristic must support notify and write with response",
            ));
        }
        let cccd = link.find_cccd(value + 1, end).await?;
        value_handle.store(value, Ordering::Relaxed);
        let mut subscribe = vec![0x12];
        subscribe.extend(cccd.to_le_bytes());
        subscribe.extend([1, 0]);
        link.request(&subscribe, 0x13).await?;
        link.characteristic = value;
        Ok(Arc::new(link))
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }
    pub fn max_value(&self) -> usize {
        usize::from(self.mtu.saturating_sub(3))
    }
    pub async fn take_notifications(&self) -> io::Result<mpsc::Receiver<Vec<u8>>> {
        self.notifications
            .lock()
            .await
            .take()
            .ok_or_else(|| invalid("notifications already taken"))
    }
    pub async fn write(&self, frame: &[u8]) -> io::Result<()> {
        if frame.len() > self.max_value() {
            return Err(invalid("frame exceeds negotiated LE ATT MTU"));
        }
        let mut request = Vec::with_capacity(frame.len() + 3);
        request.push(0x12);
        request.extend(self.characteristic.to_le_bytes());
        request.extend(frame);
        self.request(&request, 0x13).await?;
        Ok(())
    }

    async fn request(&self, packet: &[u8], expected: u8) -> io::Result<Vec<u8>> {
        // The single receiver mutex also serializes request/response pairs.
        let mut replies = self.replies.lock().await;
        timeout(Duration::from_secs(10), write_packet(&self.socket, packet))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "ATT write timed out"))??;
        let response = timeout(Duration::from_secs(10), replies.recv())
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "ATT response timed out"))?
            .ok_or_else(|| io::Error::new(io::ErrorKind::ConnectionAborted, "LE link closed"))?;
        if let Some(error) = att_error(packet[0], &response) {
            return Err(error);
        }
        if response.first() != Some(&expected) {
            return Err(io::Error::other(format!(
                "unexpected ATT response to 0x{:02x}: expected 0x{expected:02x}, got 0x{:02x}",
                packet[0],
                response.first().copied().unwrap_or_default()
            )));
        }
        Ok(response)
    }

    async fn find_service(&self, service: Uuid) -> io::Result<(u16, u16)> {
        let mut start = 1u16;
        loop {
            let mut request = vec![0x10];
            request.extend(start.to_le_bytes());
            request.extend(u16::MAX.to_le_bytes());
            request.extend([0x00, 0x28]); // primary service declarations
            let response = self.request(&request, 0x11).await?;
            if response.len() < 8 {
                return Err(invalid("short ATT service list"));
            }
            let size = response[1] as usize;
            if !matches!(size, 6 | 20) || (response.len() - 2) % size != 0 {
                return Err(invalid("malformed ATT service list"));
            }
            let mut last = 0;
            for entry in response[2..].chunks_exact(size) {
                let first = le16(entry)?;
                let end = le16(&entry[2..])?;
                if first < start || end < first || end <= last {
                    return Err(invalid("invalid ATT service range"));
                }
                if matches_uuid(&entry[4..], service) {
                    return Ok((first, end));
                }
                last = end;
            }
            if last == u16::MAX {
                break;
            }
            start = last + 1;
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "BitChat LE service not found",
        ))
    }

    async fn find_characteristic(&self, first: u16, end: u16, uuid: Uuid) -> io::Result<(u16, u8)> {
        let mut start = first;
        loop {
            let mut request = vec![0x08];
            request.extend(start.to_le_bytes());
            request.extend(end.to_le_bytes());
            request.extend([0x03, 0x28]); // characteristic declarations
            let response = self.request(&request, 0x09).await?;
            if response.len() < 9 {
                return Err(invalid("short ATT characteristic list"));
            }
            let size = response[1] as usize;
            if !matches!(size, 7 | 21) || (response.len() - 2) % size != 0 {
                return Err(invalid("malformed ATT characteristic list"));
            }
            let mut last = 0;
            for entry in response[2..].chunks_exact(size) {
                let decl = le16(entry)?;
                let value = le16(&entry[3..])?;
                if decl < start || value <= decl || value > end || decl <= last {
                    return Err(invalid("invalid ATT characteristic range"));
                }
                if matches_uuid(&entry[5..], uuid) {
                    return Ok((value, entry[2]));
                }
                last = decl;
            }
            if last >= end {
                break;
            }
            start = last + 1;
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "BitChat LE characteristic not found",
        ))
    }

    async fn find_cccd(&self, first: u16, end: u16) -> io::Result<u16> {
        if first > end {
            return Err(invalid("no room for BitChat notify descriptor"));
        }
        let mut start = first;
        loop {
            let mut request = vec![0x04];
            request.extend(start.to_le_bytes());
            request.extend(end.to_le_bytes());
            let response = self.request(&request, 0x05).await?;
            if response.len() < 6 {
                return Err(invalid("short ATT descriptor list"));
            }
            let size = match response[1] {
                1 => 4,
                2 => 18,
                _ => return Err(invalid("invalid ATT descriptor format")),
            };
            if (response.len() - 2) % size != 0 {
                return Err(invalid("malformed ATT descriptor list"));
            }
            let mut last = 0;
            for entry in response[2..].chunks_exact(size) {
                let handle = le16(entry)?;
                if handle < start || handle > end || handle <= last {
                    return Err(invalid("invalid ATT descriptor range"));
                }
                if size == 4 && entry[2..] == [0x02, 0x29] {
                    return Ok(handle);
                }
                last = handle;
            }
            if last >= end {
                break;
            }
            start = last + 1;
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "BitChat LE notify descriptor not found",
        ))
    }
}

impl Drop for DirectAtt {
    fn drop(&mut self) {
        self.connected.store(false, Ordering::Release);
        self.read_task.abort();
        // The reader holds an Arc to the socket; shut it down explicitly so
        // dropping the chat immediately releases the LE link, even on errors.
        unsafe { libc::shutdown(self.socket.get_ref().as_raw_fd(), libc::SHUT_RDWR) };
    }
}

async fn connect_socket(address: [u8; 6]) -> io::Result<AsyncFd<OwnedFd>> {
    // The local ATT-CID bind is essential: without it Linux rejects the
    // connect with EINVAL even when the iPhone is advertising.
    let raw = unsafe {
        libc::socket(
            libc::AF_BLUETOOTH,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            0,
        )
    };
    if raw < 0 {
        return Err(socket_setup_error(
            "socket creation",
            io::Error::last_os_error(),
        ));
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    let local = L2Addr {
        family: libc::AF_BLUETOOTH as _,
        psm: 0,
        address: [0; 6],
        cid: ATT_CID.to_le(),
        address_type: LE_PUBLIC,
    };
    let target = L2Addr {
        family: libc::AF_BLUETOOTH as _,
        psm: 0,
        address: [0; 6],
        cid: ATT_CID.to_le(),
        address_type: LE_PUBLIC,
    };
    let mut target = target;
    for (dest, source) in target.address.iter_mut().zip(address.iter().rev()) {
        *dest = *source;
    }
    if unsafe {
        libc::bind(
            fd.as_raw_fd(),
            &local as *const _ as *const _,
            size_of::<L2Addr>() as _,
        )
    } < 0
    {
        return Err(socket_setup_error("local bind", io::Error::last_os_error()));
    }
    let socket =
        AsyncFd::new(fd).map_err(|error| socket_setup_error("readiness registration", error))?;
    let result = unsafe {
        libc::connect(
            socket.get_ref().as_raw_fd(),
            &target as *const _ as *const _,
            size_of::<L2Addr>() as _,
        )
    };
    if result < 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EINPROGRESS) {
            return Err(socket_setup_error("connect", error));
        }
    }
    let mut guard = socket
        .writable()
        .await
        .map_err(|error| socket_setup_error("connect readiness", error))?;
    guard.clear_ready();
    let mut error = 0;
    let mut len = size_of::<libc::c_int>() as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            socket.get_ref().as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_ERROR,
            &mut error as *mut _ as *mut _,
            &mut len,
        )
    } < 0
    {
        return Err(socket_setup_error(
            "connection status query",
            io::Error::last_os_error(),
        ));
    }
    if error != 0 {
        return Err(socket_setup_error(
            "connection completion",
            io::Error::from_raw_os_error(error),
        ));
    }
    Ok(socket)
}

async fn write_packet(socket: &AsyncFd<OwnedFd>, bytes: &[u8]) -> io::Result<()> {
    loop {
        let mut ready = socket.writable().await?;
        match ready.try_io(|fd| {
            let n = unsafe {
                libc::send(
                    fd.as_raw_fd(),
                    bytes.as_ptr() as *const _,
                    bytes.len(),
                    libc::MSG_NOSIGNAL,
                )
            };
            if n < 0 {
                Err(io::Error::last_os_error())
            } else if n as usize != bytes.len() {
                Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "partial ATT frame",
                ))
            } else {
                Ok(())
            }
        }) {
            Ok(result) => return result,
            Err(_) => continue,
        }
    }
}

async fn read_packet(socket: &AsyncFd<OwnedFd>) -> io::Result<Vec<u8>> {
    loop {
        let mut ready = socket.readable().await?;
        match ready.try_io(|fd| {
            let mut bytes = vec![0u8; MAX_ATT];
            let n =
                unsafe { libc::recv(fd.as_raw_fd(), bytes.as_mut_ptr() as *mut _, bytes.len(), 0) };
            if n < 0 {
                return Err(io::Error::last_os_error());
            }
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "LE ATT socket closed",
                ));
            }
            bytes.truncate(n as usize);
            Ok(bytes)
        }) {
            Ok(result) => return result,
            Err(_) => continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connection_failure_keeps_stage_kind_and_underlying_errno() {
        let error = socket_setup_error(
            "connection completion",
            io::Error::from_raw_os_error(libc::ENOSYS),
        );
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert_eq!(setup_errno(&error), Some(libc::ENOSYS));
        assert!(error.to_string().contains("connection completion"));
        assert!(error.to_string().contains("os error 38"));
        assert_eq!(
            setup_errno(&io::Error::from_raw_os_error(libc::ENODEV)),
            Some(libc::ENODEV)
        );
    }
    #[test]
    fn mtu_exchange_preserves_text_default_and_reports_actual_file_capability() {
        assert_eq!(
            negotiated_att_mtu(TEXT_ATT_MTU, &[0x03, 0x05, 0x02]).unwrap(),
            185
        );
        assert_eq!(
            negotiated_att_mtu(FILE_ATT_MTU, &[0x03, 0x05, 0x02]).unwrap(),
            517
        );
        assert_eq!(
            negotiated_att_mtu(FILE_ATT_MTU, &[0x03, 0xb9, 0]).unwrap(),
            185
        );
        assert!(negotiated_att_mtu(FILE_ATT_MTU, &[0x03, 0x06, 0x02]).is_err());
        assert!(negotiated_att_mtu(FILE_ATT_MTU, &[0x03, 0x05]).is_err());
        assert!(negotiated_att_mtu(FILE_ATT_MTU, &[0x03, 0, 0]).is_err());
    }

    #[test]
    fn uuid_and_ranges_are_strict() {
        let uuid = Uuid::parse_str("f47b5e2d-4a9e-4c5a-9b3f-8e1d2c3a4b5c").unwrap();
        let reversed: Vec<u8> = uuid.as_bytes().iter().rev().copied().collect();
        assert!(matches_uuid(&reversed, uuid));
        assert!(!matches_uuid(uuid.as_bytes(), uuid));
        assert!(!matches_uuid(&reversed[..15], uuid));
        assert!(le16(&[3]).is_err());
    }
    #[test]
    fn closed_app_is_a_clear_retryable_gatt_error() {
        let error = att_error(0x10, &[0x01, 0x10, 0x83, 0x00, 0x0a]).unwrap();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(error.to_string().contains("open BitChat"));
        assert_eq!(
            att_error(0x10, &[0x01, 0x08, 0, 0, 0x0a]).unwrap().kind(),
            io::ErrorKind::InvalidData
        );
        assert!(att_error(0x10, &[0x11, 0x06]).is_none());
    }

    #[test]
    fn incoming_iphone_request_does_not_steal_our_response() {
        assert_eq!(
            incoming_att_request(&[0x12, 0x0b, 0x00, 0x02, 0x00], TEXT_ATT_MTU),
            Some(vec![0x01, 0x12, 0x0b, 0x00, 0x01])
        );
        assert_eq!(
            incoming_att_request(&[0x02, 0x17, 0x00], TEXT_ATT_MTU),
            Some(vec![0x03, 0xb9, 0x00])
        );
        assert_eq!(
            incoming_att_request(&[0x02, 0xb9, 0x00], FILE_ATT_MTU),
            Some(vec![0x03, 0x05, 0x02])
        );
        assert!(incoming_att_request(&[0x03, 0xb9, 0x00], TEXT_ATT_MTU).is_none());
        assert!(incoming_att_request(&[0x11, 0x06], TEXT_ATT_MTU).is_none());
        assert!(incoming_att_request(&[0x12], TEXT_ATT_MTU).is_none());
    }

    #[test]
    fn linux_att_address_layout_matches_socket_abi() {
        assert_eq!(size_of::<L2Addr>(), 14);
        assert_eq!(ATT_CID, 4);
    }
}
