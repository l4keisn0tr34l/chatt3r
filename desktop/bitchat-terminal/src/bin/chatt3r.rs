//! Minimal hardware interoperability harness, not a full BitChat client.
#[path = "../baseline/protocol.rs"]
mod protocol;
#[path = "../baseline/ui.rs"]
mod ui;

use btleplug::api::{
    Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use ed25519_dalek::{SigningKey, VerifyingKey};
use futures::StreamExt;
use protocol::{Packet, Reassembler, ANNOUNCE, FRAGMENT, LEAVE, MAX_PAYLOAD, MESSAGE};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, timeout, Duration};
use uuid::Uuid;
use x25519_dalek::{PublicKey, StaticSecret};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const SERVICE: Uuid = Uuid::from_u128(0xF47B5E2D_4A9E_4C5A_9B3F_8E1D2C3A4B5C);
const CHARACTERISTIC: Uuid = Uuid::from_u128(0xA1B2C3D4_E5F6_4A5B_8C9D_0E1F2A3B4C5D);

struct Options {
    name: String,
    write_limit: Option<usize>,
    scan_only: bool,
    scan_seconds: u64,
    debug: bool,
}

fn options() -> Result<Option<Options>> {
    let mut options = Options {
        name: "chatt3r-linux".into(),
        write_limit: None,
        scan_only: false,
        scan_seconds: 30,
        debug: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(
                    "chatt3r — experimental BitChat BLE public-text baseline\n\
Usage: chatt3r --write-limit <bytes> [--name <nickname>] [--debug]\n\
       chatt3r --scan-only [--scan-seconds <seconds>]\n\
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
            "--debug" | "-d" => options.debug = true,
            _ => return Err(format!("unknown argument: {arg}; use --help").into()),
        }
    }
    if options.name.is_empty() || options.name.len() > 24 {
        return Err("nickname must be 1..24 UTF-8 bytes".into());
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

async fn discover(adapter: &Adapter, scan_only: bool, debug: bool) -> Result<Option<Peripheral>> {
    let mut seen = HashSet::new();
    loop {
        for peripheral in adapter.peripherals().await? {
            if let Some(properties) = peripheral.properties().await? {
                if properties.services.contains(&SERVICE) {
                    if seen.insert(peripheral.id()) && (scan_only || debug) {
                        println!(
                            "[scan] BitChat service found; name={}",
                            display(properties.local_name.as_deref().unwrap_or("(unnamed)"))
                        );
                    }
                    if !scan_only {
                        return Ok(Some(peripheral));
                    }
                }
            }
        }
        sleep(Duration::from_millis(500)).await;
    }
}

async fn send(
    peripheral: &Peripheral,
    characteristic: &Characteristic,
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
    let write_type = if characteristic.properties.contains(CharPropFlags::WRITE) {
        WriteType::WithResponse
    } else {
        WriteType::WithoutResponse
    };
    for frame in frames {
        timeout(
            Duration::from_secs(10),
            peripheral.write(characteristic, &frame, write_type),
        )
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

async fn connect_with_retry(peripheral: &Peripheral, debug: bool) -> Result<()> {
    for attempt in 1..=3 {
        if debug {
            println!("[ble] connecting (attempt {attempt}/3)");
        }
        let result = timeout(Duration::from_secs(20), peripheral.connect()).await;
        let error: Box<dyn Error> = match result {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(error)) if !transient_connect_error(&error) => return Err(error.into()),
            Ok(Err(error)) => error.into(),
            Err(error) => error.into(),
        };
        eprintln!("[ble] connection attempt {attempt} failed: {error}");
        // Cancel only this peer's pending connection, never restart the adapter
        // or alter pairing. A dropped/timed-out D-Bus future doesn't cancel BlueZ.
        let _ = timeout(Duration::from_secs(5), peripheral.disconnect()).await;
        if attempt == 3 {
            return Err(error);
        }
        println!("[ble] cleared this peer's pending connection; retrying in 2s");
        sleep(Duration::from_secs(2)).await;
    }
    unreachable!()
}

async fn chat(peripheral: &Peripheral, options: &Options) -> Result<()> {
    if options.debug {
        println!("[ble] connection established; discovering GATT services");
    }
    timeout(Duration::from_secs(15), peripheral.discover_services())
        .await
        .map_err(|_| "GATT service discovery timed out after 15s")??;
    let characteristics = peripheral.characteristics();
    if options.debug {
        println!(
            "[ble] GATT discovery complete; {} characteristics",
            characteristics.len()
        );
    }
    let characteristic = characteristics
        .iter()
        .find(|c| c.uuid == CHARACTERISTIC && c.service_uuid == SERVICE)
        .ok_or("BitChat characteristic not found")?;
    if !characteristic.properties.contains(CharPropFlags::NOTIFY)
        || !characteristic
            .properties
            .intersects(CharPropFlags::WRITE | CharPropFlags::WRITE_WITHOUT_RESPONSE)
    {
        return Err("characteristic lacks notify/write properties".into());
    }
    let mut notifications = peripheral.notifications().await?;
    if options.debug {
        println!(
            "[ble] subscribing to BitChat notifications; properties={:?}",
            characteristic.properties
        );
    }
    timeout(
        Duration::from_secs(10),
        peripheral.subscribe(characteristic),
    )
    .await.map_err(|_| "BitChat notification subscription timed out after 10s; keep iPhone unlocked with BitChat in foreground")??;
    if options.debug {
        println!("[ble] connected and subscribed; characteristic={CHARACTERISTIC}");
    }
    let secret = StaticSecret::random_from_rng(OsRng);
    let noise_public = PublicKey::from(&secret).to_bytes();
    let local = protocol::peer_id(&noise_public);
    let signing = SigningKey::generate(&mut OsRng);
    let announce_payload =
        protocol::announcement(&options.name, &noise_public, &signing.verifying_key())?;
    let limit = options.write_limit.unwrap();
    if options.debug {
        println!(
            "[identity] {} ({}) — ephemeral for this run",
            display(&options.name),
            hex::encode(local)
        );
        println!("[ble] configured write limit={limit} bytes; negotiated MTU unknown");
    }
    println!(
        "Connected as {}. Public chat — not encrypted; max {MAX_PAYLOAD} UTF-8 bytes.",
        display(&options.name)
    );
    let mut receiver = Receiver {
        local,
        assembler: Reassembler::default(),
        peers: HashMap::new(),
        seen: HashSet::new(),
        order: VecDeque::new(),
    };
    let mut initial_announce = Packet::new(ANNOUNCE, local, now_ms(), announce_payload.clone());
    initial_announce.sign(&signing)?;
    send(
        peripheral,
        characteristic,
        &initial_announce,
        limit,
        &ui::Output::plain(options.debug),
    )
    .await?;
    println!("/peers · /announce · /quit");
    let (mut input, output) = ui::open(options.debug)?;
    let mut announce_tick = tokio::time::interval(Duration::from_secs(15));
    announce_tick.tick().await; // Initial announcement already sent.
    let mut link_tick = tokio::time::interval(Duration::from_secs(2));
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = link_tick.tick() => {
                if !peripheral.is_connected().await? { return Err("BLE disconnected; rerun to reconnect".into()); }
            }
            _ = announce_tick.tick() => {
                let mut packet = Packet::new(ANNOUNCE, local, now_ms(), announce_payload.clone());
                packet.sign(&signing)?;
                send(peripheral, characteristic, &packet, limit, &output).await?;
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
                if receiver.peers.is_empty() { output.line("Waiting for a peer announcement. Keep BitChat's Bluetooth room open, then resend.")?; continue; }
                if line.len() > MAX_PAYLOAD { output.line(&format!("Limit: {MAX_PAYLOAD} UTF-8 bytes; longer/compressed text not implemented."))?; continue; }
                let sent_text = display(&line);
                let mut packet = Packet::new(MESSAGE, local, now_ms(), line.into_bytes());
                packet.sign(&signing)?;
                send(peripheral, characteristic, &packet, limit, &output).await?;
                output.line(&format!("[{}] {sent_text}", output.own()))?;
                output.diagnostic("[tx] GATT write complete; verify receipt on iPhone (no application ACK).")?;
            }
            notification = notifications.next() => {
                let Some(notification) = notification else { return Err("notification stream ended; rerun to reconnect".into()); };
                if notification.uuid != CHARACTERISTIC { continue; }
                output.diagnostic(&format!("[rx] value_bytes={} type={:?}", notification.value.len(), notification.value.get(1)))?;
                if let Err(error) = receiver.receive(&notification.value, &output) { output.diagnostic(&format!("[drop] {error}"))?; }
            }
        }
    }
    let mut leave = Packet::new(LEAVE, local, now_ms(), vec![]);
    leave.sign(&signing)?;
    let _ = send(peripheral, characteristic, &leave, limit, &output).await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let Some(options) = options()? else {
        return Ok(());
    };
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
    adapter
        .start_scan(ScanFilter {
            services: vec![SERVICE],
        })
        .await?;
    if options.debug || options.scan_only {
        println!(
            "[scan] service={SERVICE}; duration={}s; no pairing required",
            options.scan_seconds
        );
    } else {
        println!("Looking for nearby BitChat peers…");
    }
    let discovered = tokio::select! {
        _ = tokio::signal::ctrl_c() => Ok(None),
        result = timeout(Duration::from_secs(options.scan_seconds), discover(&adapter, options.scan_only, options.debug)) => {
            match result { Ok(result) => result, Err(_) => {
                if options.debug || options.scan_only { println!("[scan] duration ended"); }
                Ok(None)
            } }
        }
    };
    adapter.stop_scan().await?;
    let Some(peripheral) = discovered? else {
        if !options.scan_only {
            return Err(
                "no BitChat peer connected; keep iPhone app open, check permissions, rerun".into(),
            );
        }
        return Ok(());
    };
    let connection = tokio::select! {
        _ = tokio::signal::ctrl_c() => Err("connection cancelled".into()),
        result = connect_with_retry(&peripheral, options.debug) => result,
    };
    let result = match connection {
        Ok(()) => chat(&peripheral, &options).await,
        Err(error) => Err(error),
    };
    let _ = timeout(Duration::from_secs(5), peripheral.disconnect()).await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn terminal_controls_are_escaped() {
        assert_eq!(display("a\x1b[31m\n"), "a\\u{1b}[31m\\n");
    }
}
