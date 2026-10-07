//! Short public-text subset of BitChat, pinned in docs/upstream-analysis.md.
//! No legacy crypto, compression, Noise, file transfer, or mesh forwarding.
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const ANNOUNCE: u8 = 0x01;
pub const MESSAGE: u8 = 0x02;
pub const LEAVE: u8 = 0x03;
pub const FRAGMENT: u8 = 0x20;
pub const MAX_PAYLOAD: usize = 99; // Constants.swift sets compressionThresholdBytes = 100.
const MAX_FRAME: usize = 16 * 1024;

/// Metadata-only frame shape for diagnosing rejected notifications. Never
/// include sender, recipient, fragment ID, content or a signature in logs.
/// In particular, `actual < expected` only proves incomplete framing, not
/// whether the peer sent too much, ATT capped it, or another link intervened.
pub fn frame_shape(bytes: &[u8]) -> String {
    let header = match bytes.first() {
        Some(1) => 14,
        Some(2) => 16,
        _ => return format!("unrecognized version; actual={}", bytes.len()),
    };
    if bytes.len() < header + 8 {
        return format!("short header; version={} actual={}", bytes[0], bytes.len());
    }
    let flags = bytes[11];
    let length = if header == 14 {
        u16::from_be_bytes(bytes[12..14].try_into().unwrap()) as usize
    } else {
        u32::from_be_bytes(bytes[12..16].try_into().unwrap()) as usize
    };
    let payload_start = header + 8 + if flags & 1 != 0 { 8 } else { 0 };
    let expected = payload_start
        .checked_add(length)
        .and_then(|n| n.checked_add(if flags & 2 != 0 { 64 } else { 0 }));
    let mut shape = format!("version={} kind=0x{:02x} flags=0x{flags:02x} declared_payload={length} actual={} expected={expected:?}", bytes[0], bytes[1], bytes.len());
    // If the unsigned 0x20 fragment prefix survived, its index and original
    // type distinguish media from announceV2 without disclosing the payload.
    if bytes[1] == FRAGMENT && flags & !0x03 == 0 {
        if let Some(meta) = bytes.get(payload_start..payload_start.saturating_add(13)) {
            let index = u16::from_be_bytes(meta[8..10].try_into().unwrap());
            let total = u16::from_be_bytes(meta[10..12].try_into().unwrap());
            shape.push_str(&format!(
                " fragment={index}/{total} original_type=0x{:02x}",
                meta[12]
            ));
        }
    }
    shape
}

#[derive(Clone, Debug, PartialEq)]
pub struct Packet {
    pub version: u8,
    pub kind: u8,
    pub ttl: u8,
    pub timestamp: u64,
    pub sender: [u8; 8],
    pub recipient: Option<[u8; 8]>,
    pub payload: Vec<u8>,
    pub signature: Option<[u8; 64]>,
}

impl Packet {
    pub fn new(kind: u8, sender: [u8; 8], timestamp: u64, payload: Vec<u8>) -> Self {
        Self {
            version: 1,
            kind,
            ttl: 7,
            timestamp,
            sender,
            recipient: None,
            payload,
            signature: None,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, &'static str> {
        if !matches!(self.version, 1 | 2) || self.payload.len() > MAX_FRAME {
            return Err("unsupported version or oversized payload");
        }
        let mut bytes = vec![self.version, self.kind, self.ttl];
        bytes.extend(self.timestamp.to_be_bytes());
        bytes.push(u8::from(self.recipient.is_some()) | (u8::from(self.signature.is_some()) << 1));
        if self.version == 1 {
            bytes.extend((self.payload.len() as u16).to_be_bytes());
        } else {
            bytes.extend((self.payload.len() as u32).to_be_bytes());
        }
        bytes.extend(self.sender);
        if let Some(recipient) = self.recipient {
            bytes.extend(recipient);
        }
        bytes.extend(&self.payload);
        if let Some(signature) = self.signature {
            bytes.extend(signature);
        }
        Ok(bytes) // Public on-wire packets need not be padded.
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 22 || bytes.len() > MAX_FRAME {
            return Err("invalid frame size");
        }
        let header = match bytes[0] {
            1 => 14,
            2 => 16,
            _ => return Err("unsupported version"),
        };
        if bytes.len() < header + 8 {
            return Err("truncated sender");
        }
        let flags = bytes[11];
        // A deliberately narrow harness: don't silently misparse compressed or routed frames.
        if flags & !3 != 0 {
            return Err("compressed/routed/extended flags unsupported in baseline");
        }
        let length = if header == 14 {
            u16::from_be_bytes(bytes[12..14].try_into().unwrap()) as usize
        } else {
            u32::from_be_bytes(bytes[12..16].try_into().unwrap()) as usize
        };
        if length > MAX_FRAME {
            return Err("declared payload exceeds baseline cap");
        }
        let payload_start = header + 8 + if flags & 1 != 0 { 8 } else { 0 };
        let end = payload_start + length;
        let signature_end = end + if flags & 2 != 0 { 64 } else { 0 };
        if signature_end > bytes.len() {
            return Err("truncated packet");
        }
        let recipient = if flags & 1 != 0 {
            Some(bytes[header + 8..payload_start].try_into().unwrap())
        } else {
            None
        };
        Ok(Self {
            version: bytes[0],
            kind: bytes[1],
            ttl: bytes[2],
            timestamp: u64::from_be_bytes(bytes[3..11].try_into().unwrap()),
            sender: bytes[header..header + 8].try_into().unwrap(),
            recipient,
            payload: bytes[payload_start..end].to_vec(),
            signature: if flags & 2 != 0 {
                Some(bytes[end..signature_end].try_into().unwrap())
            } else {
                None
            },
        }) // Upstream accepts trailing padding; length fields delimit the packet.
    }

    fn signing_bytes(&self) -> Result<Vec<u8>, &'static str> {
        if self.payload.len() > MAX_PAYLOAD {
            return Err("baseline only signs short uncompressed payloads");
        }
        let mut unsigned = self.clone();
        unsigned.ttl = 0;
        unsigned.signature = None;
        let mut bytes = unsigned.encode()?;
        // Swift toBinaryDataForSigning uses BinaryProtocol.encode with default padding.
        if let Some(bucket) = [256, 512, 1024, 2048]
            .into_iter()
            .find(|n| bytes.len() + 16 <= *n)
        {
            let pad = bucket - bytes.len();
            if pad <= 255 {
                bytes.resize(bucket, pad as u8);
            }
        }
        Ok(bytes)
    }

    pub fn sign(&mut self, key: &SigningKey) -> Result<(), &'static str> {
        self.signature = Some(key.sign(&self.signing_bytes()?).to_bytes());
        Ok(())
    }

    pub fn verify(&self, key: &VerifyingKey) -> Result<(), &'static str> {
        let signature = Signature::from_bytes(&self.signature.ok_or("missing signature")?);
        key.verify_strict(&self.signing_bytes()?, &signature)
            .map_err(|_| "invalid signature")
    }

    pub fn is_broadcast(&self) -> bool {
        self.recipient.is_none() || self.recipient == Some([0xff; 8])
    }
}

pub fn peer_id(noise_public: &[u8; 32]) -> [u8; 8] {
    Sha256::digest(noise_public)[..8].try_into().unwrap()
}

pub fn announcement(
    nickname: &str,
    noise_public: &[u8; 32],
    signing: &VerifyingKey,
) -> Result<Vec<u8>, &'static str> {
    if nickname.is_empty() || nickname.len() > 24 {
        return Err("nickname must be 1..24 UTF-8 bytes");
    }
    let mut bytes = vec![1, nickname.len() as u8];
    bytes.extend(nickname.as_bytes());
    bytes.extend([2, 32]);
    bytes.extend(noise_public);
    bytes.extend([3, 32]);
    bytes.extend(signing.to_bytes());
    Ok(bytes)
}

pub fn decode_announcement(packet: &Packet) -> Result<(String, VerifyingKey), &'static str> {
    if packet.payload.len() > MAX_PAYLOAD {
        return Err("oversized announcement for baseline");
    }
    let mut nickname = None;
    let mut noise = None;
    let mut signing = None;
    let mut offset = 0;
    while offset < packet.payload.len() {
        let header = packet
            .payload
            .get(offset..offset + 2)
            .ok_or("truncated TLV")?;
        let kind = header[0];
        let length = header[1] as usize;
        offset += 2;
        let value = packet
            .payload
            .get(offset..offset + length)
            .ok_or("truncated TLV value")?;
        offset += length;
        match kind {
            1 if nickname.is_none() => {
                nickname = Some(
                    std::str::from_utf8(value)
                        .map_err(|_| "invalid nickname UTF-8")?
                        .to_owned(),
                )
            }
            2 if noise.is_none() => {
                noise = Some(<[u8; 32]>::try_from(value).map_err(|_| "invalid Noise key length")?)
            }
            3 if signing.is_none() => {
                signing =
                    Some(<[u8; 32]>::try_from(value).map_err(|_| "invalid signing key length")?)
            }
            1..=3 => return Err("duplicate identity TLV"),
            _ => {} // Unknown capabilities/TLVs are skipped, never asserted as supported.
        }
    }
    if peer_id(&noise.ok_or("missing Noise key")?) != packet.sender {
        return Err("sender/key mismatch");
    }
    let key = VerifyingKey::from_bytes(&signing.ok_or("missing signing key")?)
        .map_err(|_| "invalid signing key")?;
    packet.verify(&key)?;
    Ok((nickname.ok_or("missing nickname")?, key))
}

/// Uses the upstream 0x20 whole-packet fragmentation format, sized per operator limit.
pub fn frames(packet: &Packet, write_limit: usize) -> Result<Vec<Vec<u8>>, &'static str> {
    if !(36..=512).contains(&write_limit) {
        return Err("write limit must be 36..512 characteristic-value bytes");
    }
    let encoded = packet.encode()?;
    if encoded.len() <= write_limit {
        return Ok(vec![encoded]);
    }
    let overhead = 14 + 8 + if packet.recipient.is_some() { 8 } else { 0 } + 13;
    let chunk_size = write_limit
        .checked_sub(overhead)
        .filter(|n| *n > 0)
        .ok_or("write limit too small for fragment header")?;
    let count = encoded.len().div_ceil(chunk_size);
    if count > 256 {
        return Err("too many fragments; increase write limit");
    }
    let id: [u8; 8] = rand::random();
    encoded
        .chunks(chunk_size)
        .enumerate()
        .map(|(index, chunk)| {
            let mut payload = id.to_vec();
            payload.extend((index as u16).to_be_bytes());
            payload.extend((count as u16).to_be_bytes());
            payload.push(packet.kind);
            payload.extend(chunk);
            let mut fragment = Packet::new(FRAGMENT, packet.sender, packet.timestamp, payload);
            fragment.ttl = packet.ttl;
            fragment.recipient = packet.recipient;
            fragment.encode()
        })
        .collect()
}

struct Assembly {
    started: Instant,
    kind: u8,
    recipient: Option<[u8; 8]>,
    timestamp: u64,
    parts: Vec<Option<Vec<u8>>>,
    bytes: usize,
}

#[derive(Default)]
pub struct Reassembler {
    pending: HashMap<([u8; 8], [u8; 8]), Assembly>,
}

impl Reassembler {
    pub fn accept(&mut self, packet: Packet) -> Result<Option<Packet>, &'static str> {
        self.pending
            .retain(|_, a| a.started.elapsed() < Duration::from_secs(30));
        if packet.kind != FRAGMENT {
            return Ok(Some(packet));
        }
        if packet.payload.len() < 14 {
            return Err("truncated/empty fragment");
        }
        let id = packet.payload[..8].try_into().unwrap();
        let index = u16::from_be_bytes(packet.payload[8..10].try_into().unwrap()) as usize;
        let total = u16::from_be_bytes(packet.payload[10..12].try_into().unwrap()) as usize;
        let kind = packet.payload[12];
        if total == 0 || total > 256 || index >= total {
            return Err("invalid fragment index/count");
        }
        if !matches!(kind, ANNOUNCE | MESSAGE | LEAVE) {
            return Err("fragmented type unsupported in baseline");
        }
        let key = (packet.sender, id);
        if !self.pending.contains_key(&key) && self.pending.len() >= 16 {
            return Err("too many assemblies");
        }
        let assembly = self.pending.entry(key).or_insert_with(|| Assembly {
            started: Instant::now(),
            kind,
            recipient: packet.recipient,
            timestamp: packet.timestamp,
            parts: vec![None; total],
            bytes: 0,
        });
        if assembly.parts.len() != total
            || assembly.kind != kind
            || assembly.recipient != packet.recipient
            || assembly.timestamp != packet.timestamp
        {
            self.pending.remove(&key);
            return Err("conflicting fragment metadata");
        }
        let chunk = &packet.payload[13..];
        if let Some(existing) = &assembly.parts[index] {
            if existing != chunk {
                self.pending.remove(&key);
                return Err("conflicting duplicate fragment");
            }
            return Ok(None);
        }
        if assembly.bytes + chunk.len() > MAX_FRAME {
            self.pending.remove(&key);
            return Err("assembly too large");
        }
        assembly.bytes += chunk.len();
        assembly.parts[index] = Some(chunk.to_vec());
        if assembly.parts.iter().any(Option::is_none) {
            return Ok(None);
        }
        let assembly = self.pending.remove(&key).unwrap();
        let bytes: Vec<u8> = assembly.parts.into_iter().flatten().flatten().collect();
        let original = Packet::decode(&bytes)?;
        if original.sender != packet.sender
            || original.kind != kind
            || original.recipient != packet.recipient
            || original.timestamp != packet.timestamp
        {
            return Err("reassembled packet identity/type mismatch");
        }
        Ok(Some(original))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text() -> Packet {
        Packet::new(MESSAGE, [0x11; 8], 0x0102030405060708, b"hello".to_vec())
    }

    #[test]
    fn rejected_fragment_diagnostics_expose_shape_not_packet_bytes() {
        let mut fragment = Packet::new(
            FRAGMENT,
            [0x99; 8],
            123,
            [
                vec![0xab; 8],
                0_u16.to_be_bytes().to_vec(),
                3_u16.to_be_bytes().to_vec(),
                vec![0x22],
                vec![0xcc; 150],
            ]
            .concat(),
        );
        let original = fragment.encode().unwrap();
        // An incoming notification with an intact header/prefix but an
        // incomplete body must not be silently treated as a 182-byte frame.
        let clipped = &original[..182];
        assert!(Packet::decode(clipped).is_err());
        let info = frame_shape(clipped);
        assert!(info.contains("actual=182"));
        assert!(info.contains("expected=Some(185)"));
        assert!(info.contains("fragment=0/3 original_type=0x22"));
        assert!(!info.contains("abababab")); // fragment ID/content never printed
        assert!(!info.contains("99999999")); // sender never printed
        fragment.payload[0] = 0xcd;
        assert_eq!(info, frame_shape(clipped)); // decoding shape is read-only
        assert!(frame_shape(&[]).contains("unrecognized version"));
        assert!(frame_shape(&[1, FRAGMENT]).contains("short header"));
        let mut routed = original.clone();
        routed[11] = 0x08;
        assert!(frame_shape(&routed).contains("flags=0x08"));
        assert!(!frame_shape(&routed).contains("original_type="));
    }

    #[test]
    fn exact_v1_layout() {
        let packet = text();
        assert_eq!(
            hex::encode(packet.encode().unwrap()),
            "0102070102030405060708000005111111111111111168656c6c6f"
        );
        assert_eq!(Packet::decode(&packet.encode().unwrap()).unwrap(), packet);
        let mut v2 = packet.clone();
        v2.version = 2;
        v2.recipient = Some([0xff; 8]);
        assert_eq!(Packet::decode(&v2.encode().unwrap()).unwrap(), v2);
    }

    #[test]
    fn signing_padding_and_mutable_ttl() {
        let key = SigningKey::from_bytes(&[0x42; 32]);
        let mut packet = text();
        let input = packet.signing_bytes().unwrap();
        assert_eq!(input.len(), 256);
        assert_eq!(input[2], 0);
        assert!(input[27..].iter().all(|b| *b == 229));
        packet.sign(&key).unwrap();
        packet.ttl = 1;
        packet.verify(&key.verifying_key()).unwrap();
        packet.payload[0] ^= 1;
        assert!(packet.verify(&key.verifying_key()).is_err());
    }

    #[test]
    fn pinned_independent_vector() {
        let vector: serde_json::Value =
            serde_json::from_str(include_str!("../../test-vectors/public-text-v1.json")).unwrap();
        let bytes = hex::decode(vector["wire_hex"].as_str().unwrap()).unwrap();
        let packet = Packet::decode(&bytes).unwrap();
        let key_bytes: [u8; 32] = hex::decode(vector["signing_public_key_hex"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        packet
            .verify(&VerifyingKey::from_bytes(&key_bytes).unwrap())
            .unwrap();
        let mut expected =
            hex::decode(vector["signing_input_prefix_hex"].as_str().unwrap()).unwrap();
        expected.extend(std::iter::repeat_n(
            vector["signing_padding_byte"].as_u64().unwrap() as u8,
            vector["signing_padding_count"].as_u64().unwrap() as usize,
        ));
        assert_eq!(packet.signing_bytes().unwrap(), expected);
        assert_eq!(packet.payload, b"hello");
    }

    #[test]
    fn announcement_validation() {
        let key = SigningKey::from_bytes(&[0x42; 32]);
        let noise = [0x19; 32];
        let mut packet = Packet::new(
            ANNOUNCE,
            peer_id(&noise),
            1,
            announcement("linux", &noise, &key.verifying_key()).unwrap(),
        );
        packet.sign(&key).unwrap();
        assert_eq!(decode_announcement(&packet).unwrap().0, "linux");
        packet.sender[0] ^= 1;
        assert!(decode_announcement(&packet).is_err());
    }

    #[test]
    fn fragmentation_roundtrip_and_duplicates() {
        let key = SigningKey::from_bytes(&[0x42; 32]);
        let mut packet = text();
        packet.sign(&key).unwrap();
        for limit in [36, 64, 128, 244, 512] {
            let frames = frames(&packet, limit).unwrap();
            assert!(frames.iter().all(|f| f.len() <= limit));
            let mut collector = Reassembler::default();
            let mut result = None;
            let count = frames.len();
            for (index, bytes) in frames.into_iter().rev().enumerate() {
                let part = Packet::decode(&bytes).unwrap();
                let received = collector.accept(part.clone()).unwrap();
                if index + 1 < count {
                    assert!(received.is_none());
                    assert!(collector.accept(part).unwrap().is_none());
                }
                if let Some(p) = received {
                    result = Some(p);
                }
            }
            assert_eq!(result.unwrap(), packet);
        }
    }

    #[test]
    fn rejects_conflicting_fragments_and_caps() {
        let mut packet = text();
        packet.sign(&SigningKey::from_bytes(&[0x42; 32])).unwrap();
        let parts = frames(&packet, 40).unwrap();
        let first = Packet::decode(&parts[0]).unwrap();
        let mut collector = Reassembler::default();
        assert!(collector.accept(first.clone()).unwrap().is_none());
        let mut conflict = first.clone();
        conflict.payload[13] ^= 1;
        assert!(collector.accept(conflict).is_err());
        assert!(collector.pending.is_empty());
        for total in [0_u16, 257, u16::MAX] {
            let mut invalid = first.clone();
            invalid.payload[10..12].copy_from_slice(&total.to_be_bytes());
            assert!(collector.accept(invalid).is_err());
        }
        let mut invalid = first.clone();
        invalid.payload[8..10].copy_from_slice(&u16::MAX.to_be_bytes());
        assert!(collector.accept(invalid).is_err());
        for id in 0..16 {
            let mut fragment = first.clone();
            fragment.payload[0] = id;
            assert!(collector.accept(fragment).unwrap().is_none());
        }
        let mut overflow = first.clone();
        overflow.payload[0] = 16;
        assert!(collector.accept(overflow).is_err());
        for assembly in collector.pending.values_mut() {
            assembly.started = Instant::now() - Duration::from_secs(31);
        }
        assert!(collector.accept(first).unwrap().is_none());
        assert_eq!(collector.pending.len(), 1);
    }

    #[test]
    fn signatures_required_and_short_limit_enforced() {
        let key = SigningKey::from_bytes(&[0x42; 32]);
        let mut packet = text();
        assert!(packet.verify(&key.verifying_key()).is_err());
        packet.payload = vec![b'a'; MAX_PAYLOAD];
        packet.sign(&key).unwrap();
        packet.verify(&key.verifying_key()).unwrap();
        packet.payload.push(b'a');
        assert!(packet.sign(&key).is_err());
        packet.payload = b"hi".to_vec();
        packet.recipient = Some([0xff; 8]);
        packet.sign(&key).unwrap();
        let mut collector = Reassembler::default();
        let mut completed = None;
        for bytes in frames(&packet, 64).unwrap() {
            completed = collector
                .accept(Packet::decode(&bytes).unwrap())
                .unwrap()
                .or(completed);
        }
        assert_eq!(completed.unwrap(), packet);
        let encoded = packet.encode().unwrap();
        for end in 0..encoded.len() {
            assert!(Packet::decode(&encoded[..end]).is_err());
        }
    }

    #[test]
    fn malformed_inputs_never_panic() {
        let packet = text().encode().unwrap();
        for end in 0..packet.len() {
            assert!(Packet::decode(&packet[..end]).is_err());
        }
        for len in 0..1024 {
            let bytes: Vec<u8> = (0..len).map(|_| rand::random()).collect();
            if let Ok(p) = Packet::decode(&bytes) {
                let _ = Reassembler::default().accept(p);
            }
        }
        let mut bad = packet.clone();
        bad[0] = 2;
        bad[12..16].fill(0xff);
        assert!(Packet::decode(&bad).is_err());
        let mut bad = packet;
        bad[11] = 4;
        assert!(Packet::decode(&bad).is_err());
    }
}
