//! Minimal hardware interoperability harness, not a full BitChat client.
#[path = "../baseline/discovery.rs"]
mod discovery;
#[cfg(target_os = "linux")]
#[path = "../baseline/linux_att.rs"]
mod linux_att;
#[path = "../baseline/protocol.rs"]
mod protocol;
#[path = "../baseline/ui.rs"]
mod ui;
#[cfg(windows)]
#[path = "../baseline/windows_gatt.rs"]
mod windows_gatt;

use btleplug::api::{
    BDAddr, Central, CentralEvent, CharPropFlags, Characteristic, Manager as _, Peripheral as _,
    ScanFilter, ValueNotification, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use discovery::{Evidence, FreshScan};
use ed25519_dalek::{SigningKey, VerifyingKey};
use futures::{stream::BoxStream, StreamExt};
use protocol::{Packet, Reassembler, ANNOUNCE, FRAGMENT, LEAVE, MAX_PAYLOAD, MESSAGE};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::future::Future;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, timeout, Duration, Instant};
use uuid::Uuid;
use x25519_dalek::{PublicKey, StaticSecret};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const SERVICE: Uuid = Uuid::from_u128(0xF47B5E2D_4A9E_4C5A_9B3F_8E1D2C3A4B5C);
const CHARACTERISTIC: Uuid = Uuid::from_u128(0xA1B2C3D4_E5F6_4A5B_8C9D_0E1F2A3B4C5D);

struct Options {
    name: String,
    write_limit: Option<usize>,
    scan_only: bool,
    host: bool,
    scan_seconds: u64,
    direct_le: Option<BDAddr>,
    wait_for_peer: bool,
    debug: bool,
}

fn options() -> Result<Option<Options>> {
    parse_options(std::env::args().skip(1))
}

fn parse_options(args: impl IntoIterator<Item = String>) -> Result<Option<Options>> {
    let mut options = Options {
        name: "chatt3r-linux".into(),
        write_limit: None,
        scan_only: false,
        host: false,
        scan_seconds: 30,
        direct_le: None,
        wait_for_peer: false,
        debug: false,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(
                    "chatt3r — experimental BitChat BLE public-text baseline\n\
Usage: chatt3r --write-limit <bytes> [--name <nickname>] [--debug]\n\
       chatt3r --scan-only [--scan-seconds <seconds>]\n\
       chatt3r --host --write-limit <bytes> [--name <nickname>] (windows gatt server)\n\
       chatt3r --direct-le <phone-address> [--wait-for-peer] [--debug] (linux LE-only link)\n\
--write-limit: operator-provided characteristic value limit, NOT measured MTU (36..512).\n\
Short public messages only (99 UTF-8 bytes). No confidentiality, DMs, files or relaying.\n\
Keep stock BitChat open on iPhone in the Bluetooth/mesh public room.\n\
Commands: /peers, /announce, /quit. Ctrl-C / Ctrl-D exit. Run without sudo first.\n\
Quiet by default; --debug shows BLE diagnostics. Nicknames are colored per peer.\n\
Arrow keys edit/recall input; incoming messages preserve your draft. NO_COLOR disables colors."
                );
                return Ok(None);
            }
            "--name" => options.name = args.next().ok_or("missing nickname")?,
            "--write-limit" => {
                options.write_limit = Some(args.next().ok_or("missing write limit")?.parse()?)
            }
            "--scan-seconds" => {
                options.scan_seconds = args.next().ok_or("missing scan duration")?.parse()?
            }
            "--scan-only" => options.scan_only = true,
            "--host" => options.host = true,
            "--direct-le" => {
                options.direct_le = Some(args.next().ok_or("missing LE phone address")?.parse()?)
            }
            "--wait-for-peer" => options.wait_for_peer = true,
            "--debug" | "-d" => options.debug = true,
            _ => return Err(format!("unknown argument: {arg}; use --help").into()),
        }
    }
    if options.name.is_empty() || options.name.len() > 24 {
        return Err("nickname must be 1..24 UTF-8 bytes".into());
    }
    if options.wait_for_peer && options.direct_le.is_none() {
        return Err("--wait-for-peer requires --direct-le <phone-address>".into());
    }
    if options.direct_le.is_some() && options.scan_only {
        return Err("--direct-le connects; it cannot be combined with --scan-only".into());
    }
    if options.host && (options.scan_only || options.direct_le.is_some() || options.wait_for_peer) {
        return Err(
            "--host cannot be combined with scanning, --direct-le or --wait-for-peer".into(),
        );
    }
    if !(1..=300).contains(&options.scan_seconds) {
        return Err("scan seconds must be 1..300".into());
    }
    if !options.scan_only && !matches!(options.write_limit, Some(36..=512)) {
        return Err(
            "provide --write-limit <36..512>; btleplug cannot report a portable negotiated MTU"
                .into(),
        );
    }
    Ok(Some(options))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Never let hostile nicknames/messages inject terminal control sequences.
fn display(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            if c.is_control() {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}

async fn discover(
    adapter: &Adapter,
    events: &mut BoxStream<'_, CentralEvent>,
    scan: &mut FreshScan<btleplug::platform::PeripheralId>,
    scan_only: bool,
    debug: bool,
) -> Result<Option<Peripheral>> {
    let mut first_match = None;
    loop {
        // The adapter can enumerate cached profiles without hearing the phone.
        // Only a new device or a radio update *during this scan* makes it eligible.
        if let Ok(Some(event)) = timeout(Duration::from_millis(500), events.next()).await {
            match event {
                CentralEvent::DeviceDiscovered(id) => scan.observe(id, Evidence::DeviceDiscovered),
                CentralEvent::DeviceUpdated(id)
                | CentralEvent::ServicesAdvertisement { id, .. }
                | CentralEvent::ServiceDataAdvertisement { id, .. }
                | CentralEvent::ManufacturerDataAdvertisement { id, .. } => {
                    scan.observe(id, Evidence::RadioUpdate);
                }
                _ => {}
            }
        }
        let mut candidates = Vec::new();
        for peripheral in adapter.peripherals().await? {
            if let Some(properties) = peripheral.properties().await? {
                if let Some(rank) =
                    scan.rank(&peripheral.id(), properties.services.contains(&SERVICE))
                {
                    if scan.report_once(peripheral.id()) && (scan_only || debug) {
                        println!(
                            "[scan] live BitChat candidate; name={}",
                            display(properties.local_name.as_deref().unwrap_or("(unnamed)"))
                        );
                    }
                    candidates.push((rank, peripheral));
                }
            }
        }
        if !scan_only {
            candidates.sort_by_key(|(rank, _)| *rank);
            if candidates.is_empty() {
                first_match = None;
            } else {
                // Collect a short burst of updates so a cached/system profile
                // cannot win just because its event arrived first.
                let since = first_match.get_or_insert_with(Instant::now);
                if since.elapsed() >= Duration::from_millis(750) {
                    return Ok(candidates.into_iter().next().map(|(_, p)| p));
                }
            }
        }
    }
}

enum LinkWriter<'a> {
    BlueZ(&'a Peripheral, &'a Characteristic),
    #[cfg(target_os = "linux")]
    Direct(std::sync::Arc<linux_att::DirectAtt>),
    #[cfg(windows)]
    WindowsHost(std::sync::Arc<windows_gatt::GattHost>),
}

impl LinkWriter<'_> {
    async fn connected(&self) -> Result<bool> {
        match self {
            LinkWriter::BlueZ(peripheral, _) => Ok(peripheral.is_connected().await?),
            #[cfg(target_os = "linux")]
            LinkWriter::Direct(link) => Ok(link.is_connected()),
            #[cfg(windows)]
            LinkWriter::WindowsHost(host) => host.connected(),
        }
    }
    fn write_limit(&self, configured: usize) -> Result<usize> {
        match self {
            LinkWriter::BlueZ(_, _) => Ok(configured),
            #[cfg(target_os = "linux")]
            LinkWriter::Direct(link) => Ok(configured.min(link.max_value())),
            #[cfg(windows)]
            LinkWriter::WindowsHost(host) => Ok(configured.min(host.max_value()?)),
        }
    }
    fn wait_for_announcement(&self) -> bool {
        #[cfg(windows)]
        {
            matches!(self, LinkWriter::WindowsHost(_))
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
    async fn write(&self, frame: &[u8]) -> Result<()> {
        match self {
            LinkWriter::BlueZ(peripheral, characteristic) => {
                let write_type = if characteristic.properties.contains(CharPropFlags::WRITE) {
                    WriteType::WithResponse
                } else {
                    WriteType::WithoutResponse
                };
                peripheral.write(characteristic, frame, write_type).await?;
            }
            #[cfg(target_os = "linux")]
            LinkWriter::Direct(link) => link.write(frame).await?,
            #[cfg(windows)]
            LinkWriter::WindowsHost(host) => host.write(frame).await?,
        }
        Ok(())
    }
}

async fn send(
    writer: &LinkWriter<'_>,
    packet: &Packet,
    limit: usize,
    output: &ui::Output,
) -> Result<()> {
    let frames = protocol::frames(packet, limit)?;
    output.diagnostic(&format!(
        "[tx] type=0x{:02x} payload_bytes={} frames={} configured_limit={limit}",
        packet.kind,
        packet.payload.len(),
        frames.len()
    ))?;
    for frame in frames {
        timeout(Duration::from_secs(10), writer.write(&frame))
            .await?
            .map_err(|e| {
                format!("BLE write failed: {e}; check link and --write-limit (not measured MTU)")
            })?;
        sleep(Duration::from_millis(20)).await;
    }
    Ok(()) // GATT success is not a remote message delivery acknowledgement.
}

struct Receiver {
    local: [u8; 8],
    assembler: Reassembler,
    peers: HashMap<[u8; 8], (String, VerifyingKey)>,
    seen: HashSet<[u8; 32]>,
    order: VecDeque<[u8; 32]>,
}

impl Receiver {
    fn new(local: [u8; 8]) -> Self {
        Self {
            local,
            assembler: Reassembler::default(),
            peers: HashMap::new(),
            seen: HashSet::new(),
            order: VecDeque::new(),
        }
    }

    fn receive(&mut self, bytes: &[u8], output: &ui::Output) -> Result<()> {
        // This narrow harness doesn't decode or negotiate Noise/sync/media traffic.
        if bytes.len() >= 2 && !matches!(bytes[1], ANNOUNCE | MESSAGE | LEAVE | FRAGMENT) {
            return Ok(());
        }
        let Some(packet) = self.assembler.accept(Packet::decode(bytes)?)? else {
            return Ok(());
        };
        if packet.sender == self.local || !packet.is_broadcast() {
            return Ok(());
        }
        if now_ms().abs_diff(packet.timestamp) > 300_000 {
            return Err("stale/future packet (>5 minutes); check clocks".into());
        }
        if packet.kind == ANNOUNCE {
            let (name, key) = protocol::decode_announcement(&packet)?;
            if let Some((_, previous)) = self.peers.get(&packet.sender) {
                if previous != &key {
                    return Err("refusing signing-key change for known peer".into());
                }
            } else if self.peers.len() >= 128 {
                return Err("peer limit reached".into());
            }
            if self.peers.get(&packet.sender).map(|(n, _)| n) != Some(&name) {
                output.line(&format!("{} is nearby", output.peer(&name, &packet.sender)))?;
                output.diagnostic(&format!(
                    "[peer] {} ({}) — signed announcement, not Noise-authenticated",
                    display(&name),
                    hex::encode(packet.sender)
                ))?;
            }
            self.peers.insert(packet.sender, (name, key));
            return Ok(());
        }
        let (name, key) = self
            .peers
            .get(&packet.sender)
            .ok_or("unknown signing key; wait for peer announcement")?;
        packet.verify(key)?;
        let mut digest = Sha256::new();
        digest.update(packet.sender);
        digest.update(packet.timestamp.to_be_bytes());
        digest.update([packet.kind]);
        digest.update(&packet.payload);
        let digest: [u8; 32] = digest.finalize().into();
        if !self.seen.insert(digest) {
            return Ok(());
        }
        self.order.push_back(digest);
        if self.order.len() > 1024 {
            self.seen.remove(&self.order.pop_front().unwrap());
        }
        match packet.kind {
            MESSAGE => output.line(&output.message(
                name,
                &packet.sender,
                std::str::from_utf8(&packet.payload)?,
            ))?,
            LEAVE => {
                output.line(&format!("{} left", output.peer(name, &packet.sender)))?;
                self.peers.remove(&packet.sender);
            }
            _ => {}
        }
        Ok(())
    }
}

fn transient_connect_error(error: &btleplug::Error) -> bool {
    matches!(
        error,
        btleplug::Error::TimedOut(_) | btleplug::Error::NotConnected
    ) || error.to_string().contains("org.bluez.Error.InProgress")
        || error.to_string().contains("br-connection-busy")
}

type ReadyLink = (Characteristic, BoxStream<'static, ValueNotification>);

#[derive(Debug)]
struct StartupError {
    stage: &'static str,
    cause: Box<dyn Error>,
    retryable: bool,
    missing_service: bool,
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.stage, self.cause)
    }
}

impl Error for StartupError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

impl StartupError {
    fn service_not_ready() -> Self {
        Self {
            stage: "GATT discovery",
            cause: "BitChat characteristic unavailable; keep the iPhone app open (discovery can match cached UUIDs)".into(),
            retryable: true,
            missing_service: true,
        }
    }

    fn fatal(stage: &'static str, cause: &'static str) -> Self {
        Self {
            stage,
            cause: cause.into(),
            retryable: false,
            missing_service: false,
        }
    }
}

async fn startup_stage<T>(
    stage: &'static str,
    limit: Duration,
    operation: impl Future<Output = btleplug::Result<T>>,
) -> std::result::Result<T, StartupError> {
    let result = match timeout(limit, operation).await {
        Ok(result) => result,
        Err(_) => Err(btleplug::Error::TimedOut(limit)),
    };
    result.map_err(|cause| StartupError {
        stage,
        retryable: transient_connect_error(&cause),
        missing_service: false,
        cause: cause.into(),
    })
}

async fn prepare_link(
    peripheral: &Peripheral,
    debug: bool,
) -> std::result::Result<ReadyLink, StartupError> {
    startup_stage("connection", Duration::from_secs(20), peripheral.connect()).await?;
    if debug {
        println!("[ble] connection established; discovering GATT services");
    }
    startup_stage(
        "GATT discovery",
        Duration::from_secs(15),
        peripheral.discover_services(),
    )
    .await?;
    let characteristics = peripheral.characteristics();
    if debug {
        println!(
            "[ble] GATT discovery complete; {} characteristics",
            characteristics.len()
        );
    }
    let characteristic = characteristics
        .iter()
        .find(|c| c.uuid == CHARACTERISTIC && c.service_uuid == SERVICE)
        .ok_or_else(StartupError::service_not_ready)?
        .clone();
    if !characteristic.properties.contains(CharPropFlags::NOTIFY)
        || !characteristic
            .properties
            .intersects(CharPropFlags::WRITE | CharPropFlags::WRITE_WITHOUT_RESPONSE)
    {
        return Err(StartupError::fatal(
            "GATT discovery",
            "characteristic lacks notify/write properties",
        ));
    }
    let notifications = startup_stage(
        "notification stream",
        Duration::from_secs(5),
        peripheral.notifications(),
    )
    .await?;
    if debug {
        println!(
            "[ble] subscribing to BitChat notifications; properties={:?}",
            characteristic.properties
        );
    }
    startup_stage(
        "notification subscription",
        Duration::from_secs(10),
        peripheral.subscribe(&characteristic),
    )
    .await?;
    if debug {
        println!("[ble] connected and subscribed; characteristic={CHARACTERISTIC}");
    }
    Ok((characteristic, notifications))
}

/// Retry setup only. Once chat starts, never replay a possibly delivered message.
async fn retry_startup<T, P, D>(
    debug: bool,
    pause: Duration,
    mut prepare: impl FnMut() -> P,
    mut disconnect: impl FnMut() -> D,
) -> Result<T>
where
    P: Future<Output = std::result::Result<T, StartupError>>,
    D: Future<Output = ()>,
{
    for attempt in 1..=3 {
        if debug {
            println!("[ble] setting up link (attempt {attempt}/3)");
        }
        match prepare().await {
            Ok(link) => return Ok(link),
            Err(error) => {
                eprintln!("[ble] startup attempt {attempt} failed: {error}");
                if error.missing_service || !error.retryable || attempt == 3 {
                    return Err(error.into());
                }
                // Dropping a timed-out D-Bus future doesn't cancel BlueZ. Reset
                // only this peer's pending link, not the adapter or its pairing.
                disconnect().await;
                println!(
                    "[ble] retrying this peer in {}s; keep BitChat open and the iPhone unlocked",
                    pause.as_secs()
                );
                sleep(pause).await;
            }
        }
    }
    unreachable!()
}

async fn connect_with_retry(peripheral: &Peripheral, debug: bool) -> Result<ReadyLink> {
    retry_startup(
        debug,
        Duration::from_secs(2),
        || prepare_link(peripheral, debug),
        || async {
            let _ = timeout(Duration::from_secs(5), peripheral.disconnect()).await;
        },
    )
    .await
}

// A peripheral has no reason to notify on a link until the central has sent
// a validated peer announcement. The established phone-client path stays eager.
fn can_announce(wait_for_peer: bool, known_peers: usize) -> bool {
    !wait_for_peer || known_peers > 0
}

async fn chat(
    writer: LinkWriter<'_>,
    mut notifications: BoxStream<'static, ValueNotification>,
    options: &Options,
) -> Result<()> {
    let secret = StaticSecret::random_from_rng(OsRng);
    let noise_public = PublicKey::from(&secret).to_bytes();
    let local = protocol::peer_id(&noise_public);
    let signing = SigningKey::generate(&mut OsRng);
    let announce_payload =
        protocol::announcement(&options.name, &noise_public, &signing.verifying_key())?;
    let wait_for_announcement = writer.wait_for_announcement();
    let configured_limit = options.write_limit.unwrap();
    // A Windows subscriber can appear before its ATT MTU settles. Do not
    // query/cap notifications or transmit on that session until an inbound
    // signed announcement proves the central can actually exchange frames.
    let mut limit = if wait_for_announcement {
        configured_limit
    } else {
        writer.write_limit(configured_limit)?
    };
    if options.debug {
        println!(
            "[identity] {} ({}) — ephemeral for this run",
            display(&options.name),
            hex::encode(local)
        );
        match &writer {
            LinkWriter::BlueZ(_, _) => {
                println!("[ble] configured frame limit={limit} bytes; negotiated MTU unknown")
            }
            #[cfg(windows)]
            LinkWriter::WindowsHost(_) => {
                println!("[ble] configured notification limit={limit} bytes; checking session ATT MTU after inbound announcement")
            }
            #[cfg(target_os = "linux")]
            LinkWriter::Direct(link) => println!(
                "[ble] configured write limit={limit} bytes; negotiated ATT MTU={} bytes",
                link.max_value() + 3
            ),
        }
    }
    println!(
        "Connected as {}. Public chat — not encrypted; max {MAX_PAYLOAD} UTF-8 bytes.",
        display(&options.name)
    );
    let mut receiver = Receiver::new(local);
    let mut initial_announce = Packet::new(ANNOUNCE, local, now_ms(), announce_payload.clone());
    initial_announce.sign(&signing)?;
    let mut initial_sent = false;
    if can_announce(wait_for_announcement, receiver.peers.len()) {
        send(
            &writer,
            &initial_announce,
            limit,
            &ui::Output::plain(options.debug),
        )
        .await?;
        initial_sent = true;
    } else if options.debug {
        println!("[host] waiting for a signed peer announcement before notifying");
    }
    println!("/peers · /announce · /quit");
    let (mut input, output) = ui::open(options.debug)?;
    let mut announce_tick = tokio::time::interval(Duration::from_secs(15));
    announce_tick.tick().await; // Schedule later announcements in 15s.
    let mut link_tick = tokio::time::interval(Duration::from_secs(2));
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = link_tick.tick() => {
                if !writer.connected().await? { return Err("BLE disconnected; rerun to reconnect".into()); }
            }
            _ = announce_tick.tick() => {
                if !can_announce(wait_for_announcement, receiver.peers.len()) { continue; }
                let mut packet = Packet::new(ANNOUNCE, local, now_ms(), announce_payload.clone());
                packet.sign(&signing)?;
                send(&writer, &packet, limit, &output).await?;
            }
            line = input.next_line() => {
                let Some(line) = line? else { break; };
                if line.is_empty() { continue; }
                match line.as_str() {
                    "/quit" => break,
                    "/peers" => {
                        output.line(&format!("{} nearby peers", receiver.peers.len()))?;
                        for (id, (name, _)) in &receiver.peers {
                            let label = output.peer(name, id);
                            output.line(&if output.debug { format!("  {label} ({})", hex::encode(id)) } else { format!("  {label}") })?;
                        }
                        continue;
                    }
                    "/announce" => { announce_tick.reset_immediately(); continue; }
                    _ if line.starts_with('/') => { output.line("Unsupported command; use /peers, /announce, /quit")?; continue; }
                    _ => {}
                }
                if receiver.peers.is_empty() { output.line("Waiting for a peer announcement. Check the other peer is running, then resend.")?; continue; }
                if line.len() > MAX_PAYLOAD { output.line(&format!("Limit: {MAX_PAYLOAD} UTF-8 bytes; longer/compressed text not implemented."))?; continue; }
                let sent_text = display(&line);
                let mut packet = Packet::new(MESSAGE, local, now_ms(), line.into_bytes());
                packet.sign(&signing)?;
                send(&writer, &packet, limit, &output).await?;
                output.line(&format!("[{}] {sent_text}", output.own()))?;
                output.diagnostic("[tx] BLE frame submitted; verify receipt on the other peer (no application ACK).")?;
            }
            notification = notifications.next() => {
                let Some(notification) = notification else { return Err("notification stream ended; rerun to reconnect".into()); };
                if notification.uuid != CHARACTERISTIC { continue; }
                output.diagnostic(&format!("[rx] value_bytes={} type={:?}", notification.value.len(), notification.value.get(1)))?;
                if let Err(error) = receiver.receive(&notification.value, &output) { output.diagnostic(&format!("[drop] {error}"))?; }
                if !initial_sent && can_announce(wait_for_announcement, receiver.peers.len()) {
                    limit = writer.write_limit(configured_limit)?;
                    #[cfg(windows)]
                    if let LinkWriter::WindowsHost(host) = &writer {
                        output.diagnostic(&format!("[host] inbound signed peer; session ATT MTU={} bytes, notification limit={limit}", host.att_mtu()?))?;
                    }
                    send(&writer, &initial_announce, limit, &output).await?;
                    initial_sent = true;
                }
            }
        }
    }
    let mut leave = Packet::new(LEAVE, local, now_ms(), vec![]);
    leave.sign(&signing)?;
    let _ = send(&writer, &leave, limit, &output).await;
    Ok(())
}

#[cfg(target_os = "linux")]
fn retryable_direct_startup(error: &std::io::Error) -> bool {
    !matches!(
        error.kind(),
        std::io::ErrorKind::PermissionDenied
            | std::io::ErrorKind::InvalidInput
            | std::io::ErrorKind::InvalidData
            | std::io::ErrorKind::Unsupported
            | std::io::ErrorKind::AddrNotAvailable
    ) && !matches!(error.raw_os_error(), Some(libc::ENODEV | libc::ENETDOWN))
}

#[cfg(target_os = "linux")]
fn direct_retry_delay(attempt: u64) -> Duration {
    Duration::from_secs(match attempt {
        1 => 5,
        2 => 10,
        3 => 20,
        _ => 30,
    })
}

#[cfg(target_os = "linux")]
async fn connect_direct(
    address: BDAddr,
    wait: bool,
    debug: bool,
) -> Result<Option<std::sync::Arc<linux_att::DirectAtt>>> {
    if wait {
        println!("Waiting for the known phone's BitChat service… open the app when ready; ctrl-c cancels.");
    }
    let mut attempt = 0u64;
    loop {
        attempt = attempt.saturating_add(1);
        let result = tokio::select! {
            _ = tokio::signal::ctrl_c() => return Ok(None),
            result = linux_att::DirectAtt::connect(address.into_inner(), SERVICE, CHARACTERISTIC) => result,
        };
        match result {
            Ok(link) => return Ok(Some(link)),
            Err(error) if wait && retryable_direct_startup(&error) => {
                let pause = direct_retry_delay(attempt);
                if debug {
                    eprintln!(
                        "[ble] phone not ready (setup attempt {attempt}: {error}); retrying in {}s",
                        pause.as_secs()
                    );
                } else if attempt % 6 == 1 {
                    println!("Still waiting for BitChat on the known phone; ctrl-c cancels.");
                }
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => return Ok(None),
                    _ = sleep(pause) => {},
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let Some(options) = options()? else {
        return Ok(());
    };
    #[cfg(not(windows))]
    if options.host {
        return Err("--host requires native Windows GATT server support".into());
    }
    #[cfg(windows)]
    if options.host {
        let (host, receiver) = windows_gatt::GattHost::start().await?;
        println!("[host] Windows GATT host started; check the service in a Linux scan; waiting for one subscriber; ctrl-c cancels");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => return Ok(()),
            result = host.wait_for_client() => result?,
        }
        println!("[host] one central subscribed; exchanging signed public text frames (peer identity not yet verified)");
        let notifications: BoxStream<'static, ValueNotification> =
            futures::stream::unfold(receiver, |mut rx| async move {
                rx.recv().await.map(|value| {
                    (
                        ValueNotification {
                            uuid: CHARACTERISTIC,
                            value,
                        },
                        rx,
                    )
                })
            })
            .boxed();
        return chat(LinkWriter::WindowsHost(host), notifications, &options).await;
    }
    #[cfg(not(target_os = "linux"))]
    if options.direct_le.is_some() {
        return Err("--direct-le is Linux-only".into());
    }
    #[cfg(target_os = "linux")]
    if let Some(address) = options.direct_le {
        if options.debug {
            println!("[ble] direct LE ATT for {address}; no BlueZ profile auto-connect, pairing unchanged");
        }
        let Some(link) = connect_direct(address, options.wait_for_peer, options.debug).await?
        else {
            return Ok(());
        };
        if options.debug {
            println!("[ble] direct LE connected, GATT verified, notifications enabled; negotiated ATT MTU allows {}-byte frames", link.max_value());
        }
        let receiver = link.take_notifications().await?;
        let notifications: BoxStream<'static, ValueNotification> =
            futures::stream::unfold(receiver, |mut rx| async move {
                rx.recv().await.map(|value| {
                    (
                        ValueNotification {
                            uuid: CHARACTERISTIC,
                            value,
                        },
                        rx,
                    )
                })
            })
            .boxed();
        return chat(LinkWriter::Direct(link), notifications, &options).await;
    }
    if options.debug {
        println!("[debug] BLE diagnostics enabled; --help prints usage");
    }
    let manager = Manager::new().await?;
    let adapter = manager
        .adapters()
        .await?
        .into_iter()
        .next()
        .ok_or("no Bluetooth adapter; check bluetoothctl list")?;
    if options.debug {
        println!("[ble] adapter={}", adapter.adapter_info().await?);
    }
    // Subscribe before scanning. btleplug also emits synthetic discoveries for
    // devices already in BlueZ's cache; FreshScan ignores those until a real
    // post-start radio update arrives.
    let mut events = adapter.events().await?;
    let initial = adapter.peripherals().await?.into_iter().map(|p| p.id());
    let mut scan = FreshScan::new(initial);
    let filter = ScanFilter {
        services: vec![SERVICE],
    };
    adapter.start_scan(filter.clone()).await?;
    let deadline = Instant::now() + Duration::from_secs(options.scan_seconds);
    if options.debug || options.scan_only {
        println!(
            "[scan] service={SERVICE}; duration={}s; no pairing required",
            options.scan_seconds
        );
    } else {
        println!("Looking for nearby BitChat peers…");
    }
    let mut last_error: Option<Box<dyn Error>> = None;
    let mut cancelled = false;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let discovered = tokio::select! {
            _ = tokio::signal::ctrl_c() => { cancelled = true; Ok(None) },
            result = timeout(remaining, discover(&adapter, &mut events, &mut scan, options.scan_only, options.debug)) => {
                match result { Ok(result) => result, Err(_) => Ok(None) }
            }
        };
        let peripheral = match discovered {
            Ok(Some(peripheral)) => peripheral,
            Ok(None) => break,
            Err(error) => {
                let _ = adapter.stop_scan().await;
                return Err(error);
            }
        };
        adapter.stop_scan().await?;
        let connection = tokio::select! {
            _ = tokio::signal::ctrl_c() => Err("connection cancelled".into()),
            result = connect_with_retry(&peripheral, options.debug) => result,
        };
        if let Err(error) = &connection {
            if error
                .downcast_ref::<StartupError>()
                .is_some_and(|e| e.missing_service)
            {
                scan.reject(peripheral.id());
                eprintln!("[scan] this candidate has no active BitChat characteristic; looking for another live address");
                last_error = connection.err();
                let _ = timeout(Duration::from_secs(5), peripheral.disconnect()).await;
                if Instant::now() < deadline {
                    adapter.start_scan(filter.clone()).await?;
                    continue;
                }
                break;
            }
        }
        let result = match connection {
            Ok((characteristic, notifications)) => {
                chat(
                    LinkWriter::BlueZ(&peripheral, &characteristic),
                    notifications,
                    &options,
                )
                .await
            }
            Err(error) => Err(error),
        };
        let _ = timeout(Duration::from_secs(5), peripheral.disconnect()).await;
        return result;
    }
    let _ = adapter.stop_scan().await;
    if options.debug || options.scan_only {
        println!("[scan] duration ended");
    }
    if cancelled {
        return Ok(());
    }
    if let Some(error) = last_error {
        return Err(error);
    }
    if !options.scan_only {
        return Err("no live BitChat peer found; keep iPhone unlocked with BitChat open in its Bluetooth public room, then rerun".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_waits_for_a_validated_peer_before_initial_notification() {
        assert!(can_announce(false, 0)); // Existing phone-client behavior.
        assert!(!can_announce(true, 0));
        assert!(can_announce(true, 1));
    }

    #[test]
    fn host_mode_is_explicit_and_exclusive() {
        let args = ["--host", "--write-limit", "128"];
        let config = parse_options(args.into_iter().map(str::to_owned))
            .unwrap()
            .unwrap();
        assert!(config.host);
        assert!(!config.scan_only);
        for flags in [
            vec!["--host", "--scan-only"],
            vec!["--host", "--direct-le", "11:22:33:44:55:66"],
            vec!["--host", "--wait-for-peer"],
            vec!["--host", "--write-limit", "20"],
        ] {
            assert!(parse_options(flags.into_iter().map(str::to_owned)).is_err());
        }
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn direct_startup_wait_retries_only_missing_or_transient_links() {
        use std::io::{Error as IoError, ErrorKind};
        for kind in [
            ErrorKind::NotFound,
            ErrorKind::TimedOut,
            ErrorKind::ConnectionRefused,
            ErrorKind::ConnectionReset,
        ] {
            assert!(retryable_direct_startup(&IoError::from(kind)), "{kind:?}");
        }
        for kind in [
            ErrorKind::PermissionDenied,
            ErrorKind::InvalidInput,
            ErrorKind::InvalidData,
            ErrorKind::Unsupported,
        ] {
            assert!(!retryable_direct_startup(&IoError::from(kind)), "{kind:?}");
        }
        assert!(!retryable_direct_startup(&IoError::from_raw_os_error(
            libc::ENODEV
        )));
        assert_eq!(
            [1, 2, 3, 4, 50].map(direct_retry_delay),
            [5, 10, 20, 30, 30].map(Duration::from_secs)
        );
    }

    #[test]
    fn only_transient_connection_errors_are_retried() {
        assert!(transient_connect_error(&btleplug::Error::Other(
            "D-Bus error: br-connection-busy (org.bluez.Error.InProgress)".into()
        )));
        assert!(transient_connect_error(&btleplug::Error::TimedOut(
            Duration::from_secs(20)
        )));
        assert!(!transient_connect_error(&btleplug::Error::PermissionDenied));
        assert!(!transient_connect_error(&btleplug::Error::DeviceNotFound));
        assert!(!transient_connect_error(&btleplug::Error::Other(
            "org.bluez.Error.AuthenticationFailed".into()
        )));
    }

    #[tokio::test]
    async fn subscription_timeout_is_retryable_and_has_stage_context() {
        let error = startup_stage(
            "notification subscription",
            Duration::ZERO,
            std::future::pending::<btleplug::Result<()>>(),
        )
        .await
        .unwrap_err();
        assert!(error.retryable);
        assert!(error.to_string().contains("notification subscription"));
        assert!(error.to_string().contains("Timed out"));
    }

    #[tokio::test]
    async fn permanent_startup_errors_are_not_retried() {
        let error = startup_stage("notification subscription", Duration::from_secs(1), async {
            Err::<(), _>(btleplug::Error::PermissionDenied)
        })
        .await
        .unwrap_err();
        assert!(!error.retryable);
        assert!(
            !StartupError::fatal("GATT discovery", "unsupported characteristic properties")
                .retryable
        );
        assert!(StartupError::service_not_ready().retryable);
        let value = startup_stage("connection", Duration::from_secs(1), async { Ok(42) })
            .await
            .unwrap();
        assert_eq!(value, 42);
    }

    #[tokio::test]
    async fn startup_retries_are_bounded_and_reset_only_between_attempts() {
        use std::cell::Cell;
        for (success_on, retryable, expected_attempts, expected_resets) in [
            (Some(1), true, 1, 0),
            (Some(2), true, 2, 1),
            (None, true, 3, 2),
            (None, false, 1, 0),
        ] {
            let attempts = Cell::new(0);
            let resets = Cell::new(0);
            let result = retry_startup(
                false,
                Duration::ZERO,
                || {
                    attempts.set(attempts.get() + 1);
                    std::future::ready(if success_on == Some(attempts.get()) {
                        Ok(42)
                    } else {
                        Err(StartupError {
                            stage: "notification subscription",
                            cause: "test failure".into(),
                            retryable,
                            missing_service: false,
                        })
                    })
                },
                || {
                    resets.set(resets.get() + 1);
                    std::future::ready(())
                },
            )
            .await;
            assert_eq!(result.is_ok(), success_on.is_some());
            assert_eq!(attempts.get(), expected_attempts);
            assert_eq!(resets.get(), expected_resets);
        }
    }

    #[tokio::test]
    async fn missing_service_returns_to_scanner_without_retrying_same_peer() {
        let attempts = std::cell::Cell::new(0);
        let resets = std::cell::Cell::new(0);
        let result = retry_startup(
            false,
            Duration::ZERO,
            || {
                attempts.set(attempts.get() + 1);
                std::future::ready(Err::<(), _>(StartupError::service_not_ready()))
            },
            || {
                resets.set(resets.get() + 1);
                std::future::ready(())
            },
        )
        .await;
        assert!(
            result
                .unwrap_err()
                .downcast_ref::<StartupError>()
                .unwrap()
                .missing_service
        );
        assert_eq!(attempts.get(), 1);
        assert_eq!(resets.get(), 0); // scanner owns the per-peer disconnect in this case
    }

    #[test]
    fn two_simulated_peers_exchange_signed_text_over_fragmented_frames() {
        // This drives the real chat receiver and protocol codec, not the BLE
        // backend. It does NOT prove that two laptops can advertise/connect.
        fn deliver(receiver: &mut Receiver, packet: &Packet, output: &ui::Output) {
            for frame in protocol::frames(packet, 64).unwrap() {
                receiver.receive(&frame, output).unwrap();
            }
        }
        let output = ui::Output::plain(false);
        let key_a = SigningKey::from_bytes(&[3; 32]);
        let key_b = SigningKey::from_bytes(&[4; 32]);
        let noise_a = [7; 32];
        let noise_b = [8; 32];
        let id_a = protocol::peer_id(&noise_a);
        let id_b = protocol::peer_id(&noise_b);
        let mut a = Receiver::new(id_a);
        let mut b = Receiver::new(id_b);
        for (name, noise, id, key, remote) in [
            ("laptop-a", noise_a, id_a, &key_a, &mut b),
            ("laptop-b", noise_b, id_b, &key_b, &mut a),
        ] {
            let mut announce = Packet::new(
                ANNOUNCE,
                id,
                now_ms(),
                protocol::announcement(name, &noise, &key.verifying_key()).unwrap(),
            );
            announce.sign(key).unwrap();
            deliver(remote, &announce, &output); // 64-byte frames require fragmentation
            assert_eq!(remote.peers.get(&id).unwrap().0, name);
        }
        let mut from_a = Packet::new(MESSAGE, id_a, now_ms(), b"hello from a".to_vec());
        from_a.sign(&key_a).unwrap();
        deliver(&mut b, &from_a, &output);
        deliver(&mut b, &from_a, &output); // duplicate delivery is not printed twice
        assert_eq!(b.seen.len(), 1);
        let mut from_b = Packet::new(MESSAGE, id_b, now_ms(), b"hello from b".to_vec());
        from_b.sign(&key_b).unwrap();
        deliver(&mut a, &from_b, &output);
        assert_eq!(a.seen.len(), 1);
        let mut tampered = from_a.clone();
        tampered.payload = b"modified by relay".to_vec();
        assert!(b.receive(&tampered.encode().unwrap(), &output).is_err());
        let unsigned = Packet::new(MESSAGE, id_b, now_ms(), b"unsigned".to_vec());
        assert!(a.receive(&unsigned.encode().unwrap(), &output).is_err());
        assert_eq!(a.seen.len(), 1);
        assert_eq!(b.seen.len(), 1);
    }

    #[test]
    fn terminal_controls_are_escaped() {
        assert_eq!(display("a\x1b[31m\n"), "a\\u{1b}[31m\\n");
    }
}
