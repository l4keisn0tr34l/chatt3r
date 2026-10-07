//! Opt-in file-only whole-packet 0x20 collector. Never passes unsigned
//! fragment contents to the store; caller validates the reassembled outer frame.
use super::protocol::{Packet, FRAGMENT};
use bitchat_poc::file_wire::{inflate_fragment_payload, MAX_FILE_WIRE_BYTES};
use std::collections::HashMap;
use std::time::{Duration, Instant};

// The sender's observed 46 x 469-byte chunks exceed the old 16 KiB cap.
// Share the outer signed-frame ceiling with FileWire rather than admitting
// bytes that its bounded v2 parser cannot validate.
pub const MAX_FILE_FRAME: usize = MAX_FILE_WIRE_BYTES;
const MAX_PENDING: usize = 8;

struct Assembly {
    started: Instant,
    timestamp: u64,
    parts: Vec<Option<Vec<u8>>>,
    size: usize,
}

/// Decode only a file-marked `0x20` fragment. Stock iOS may raw-DEFLATE the
/// *fragment packet itself* (v1 flag 0x04) independently of the signed v2
/// outer file. All expanded bytes are bounded and remain untrusted until the
/// assembled outer packet is verified; non-file traffic is not reinterpreted.
pub fn decode_file_candidate(bytes: &[u8]) -> Result<Option<Packet>, &'static str> {
    if bytes.get(1) != Some(&FRAGMENT) {
        return Ok(None);
    }
    if bytes.get(11) != Some(&4) {
        let packet = Packet::decode(bytes)?;
        return Ok((packet.payload.get(12) == Some(&0x22)).then_some(packet));
    }
    let header = match bytes.first() {
        Some(1) => 14,
        Some(2) => 16,
        _ => return Err("unsupported compressed fragment version"),
    };
    let length_field = if header == 14 { 2 } else { 4 };
    if bytes.len() < header + 8 + length_field {
        return Err("truncated compressed file fragment header");
    }
    let declared = if header == 14 {
        u16::from_be_bytes(bytes[12..14].try_into().unwrap()) as usize
    } else {
        u32::from_be_bytes(bytes[12..16].try_into().unwrap()) as usize
    };
    if declared <= length_field
        || declared > 1024 + length_field
        || (header + 8).checked_add(declared) != Some(bytes.len())
    {
        return Err("truncated or oversized compressed fragment frame");
    }
    let start = header + 8;
    let original = if length_field == 2 {
        u16::from_be_bytes(bytes[start..start + 2].try_into().unwrap()) as usize
    } else {
        u32::from_be_bytes(bytes[start..start + 4].try_into().unwrap()) as usize
    };
    let payload = inflate_fragment_payload(&bytes[start + length_field..], original)?;
    if payload.get(12) != Some(&0x22) {
        return Ok(None);
    }
    let mut packet = Packet::new(
        FRAGMENT,
        bytes[header..start].try_into().unwrap(),
        u64::from_be_bytes(bytes[3..11].try_into().unwrap()),
        payload,
    );
    packet.version = bytes[0];
    packet.ttl = bytes[2];
    Ok(Some(packet))
}

#[derive(Default)]
pub struct FileFragments {
    pending: HashMap<([u8; 8], [u8; 8]), Assembly>,
}

impl FileFragments {
    /// Pass only file-marked fragments here. Outer sender/type/timestamp must
    /// also be compared with the last fragment by the caller after assembly.
    pub fn accept(&mut self, packet: &Packet) -> Result<Option<Vec<u8>>, &'static str> {
        self.pending
            .retain(|_, item| item.started.elapsed() < Duration::from_secs(30));
        if packet.kind != FRAGMENT
            || packet.payload.len() < 14
            || packet.payload[12] != 0x22
            || packet.recipient.is_some()
        {
            return Err("not a public file fragment");
        }
        let id: [u8; 8] = packet.payload[..8].try_into().unwrap();
        let index = u16::from_be_bytes(packet.payload[8..10].try_into().unwrap()) as usize;
        let count = u16::from_be_bytes(packet.payload[10..12].try_into().unwrap()) as usize;
        if count == 0 || count > 256 || index >= count {
            return Err("bad file fragment count/index");
        }
        let key = (packet.sender, id);
        if !self.pending.contains_key(&key) && self.pending.len() >= MAX_PENDING {
            return Err("too many file assemblies");
        }
        let entry = self.pending.entry(key).or_insert_with(|| Assembly {
            started: Instant::now(),
            timestamp: packet.timestamp,
            parts: vec![None; count],
            size: 0,
        });
        if entry.timestamp != packet.timestamp || entry.parts.len() != count {
            self.pending.remove(&key);
            return Err("conflicting file fragment metadata");
        }
        let chunk = &packet.payload[13..];
        if let Some(existing) = &entry.parts[index] {
            if existing != chunk {
                self.pending.remove(&key);
                return Err("conflicting file fragment bytes");
            }
            return Ok(None);
        }
        if entry.size + chunk.len() > MAX_FILE_FRAME {
            self.pending.remove(&key);
            return Err("file frame exceeds signed-file limit");
        }
        entry.size += chunk.len();
        entry.parts[index] = Some(chunk.to_vec());
        if entry.parts.iter().any(Option::is_none) {
            return Ok(None);
        }
        let complete = self.pending.remove(&key).unwrap();
        Ok(Some(
            complete.parts.into_iter().flatten().flatten().collect(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::DeflateEncoder, Compression};
    use std::io::Write;

    #[test]
    fn compressed_v1_file_fragment_inflates_before_bounded_assembly() {
        let mut payload = vec![0xab; 8];
        payload.extend(0_u16.to_be_bytes());
        payload.extend(46_u16.to_be_bytes());
        payload.push(0x22);
        payload.extend(vec![0x55; 469]);
        let original = Packet::new(FRAGMENT, [9; 8], 123, payload);
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&original.payload).unwrap();
        let deflate = encoder.finish().unwrap();
        let length = 2 + deflate.len();
        let mut wire = vec![1, FRAGMENT, original.ttl];
        wire.extend(original.timestamp.to_be_bytes());
        wire.push(4); // v1 compressed, no recipient, signature or route
        wire.extend((length as u16).to_be_bytes());
        wire.extend(original.sender);
        wire.extend((original.payload.len() as u16).to_be_bytes());
        wire.extend(&deflate);
        assert!(wire.len() < original.encode().unwrap().len());
        let decoded = decode_file_candidate(&wire).unwrap().unwrap();
        assert_eq!(decoded, original);
        let mut collector = FileFragments::default();
        assert!(collector.accept(&decoded).unwrap().is_none());
        let mut too_big = wire.clone();
        too_big[22..24].copy_from_slice(&1025_u16.to_be_bytes());
        assert!(decode_file_candidate(&too_big).is_err());
        let mut trailing = wire.clone();
        trailing.push(0);
        assert!(decode_file_candidate(&trailing).is_err()); // frame length
        trailing[12..14].copy_from_slice(&((length + 1) as u16).to_be_bytes());
        assert!(decode_file_candidate(&trailing).is_err()); // DEFLATE trailing data
        let mut altered = wire;
        altered[22..24].copy_from_slice(&(original.payload.len() as u16 - 1).to_be_bytes());
        assert!(decode_file_candidate(&altered).is_err()); // incomplete output
    }

    #[test]
    fn forty_six_stock_sized_fragments_cross_old_cap_but_stay_bounded() {
        let mut collector = FileFragments::default();
        let content = vec![0x47; 21_500]; // >16 KiB, < bounded file outer frame
        let chunks: Vec<_> = content.chunks(469).collect();
        assert_eq!(chunks.len(), 46);
        let mut completed = None;
        for (index, chunk) in chunks.iter().enumerate() {
            let mut payload = vec![3; 8];
            payload.extend((index as u16).to_be_bytes());
            payload.extend((chunks.len() as u16).to_be_bytes());
            payload.push(0x22);
            payload.extend_from_slice(chunk);
            let part = Packet::new(FRAGMENT, [2; 8], 1, payload);
            completed = collector.accept(&part).unwrap().or(completed);
        }
        assert_eq!(completed.unwrap(), content);
        // A new unsigned fragment set may not exceed the signed-file codec's
        // maximum even if it advertises a plausible 256-part count.
        let mut oversize = FileFragments::default();
        for index in 0..256_u16 {
            let mut payload = vec![4; 8];
            payload.extend(index.to_be_bytes());
            payload.extend(256_u16.to_be_bytes());
            payload.push(0x22);
            payload.extend(vec![0; 469]);
            let part = Packet::new(FRAGMENT, [2; 8], 1, payload);
            let result = oversize.accept(&part);
            if (index as usize + 1) * 469 > MAX_FILE_FRAME {
                assert!(result.is_err());
                assert!(oversize.pending.is_empty());
                break;
            }
            assert!(result.unwrap().is_none());
        }
    }

    #[test]
    fn out_of_order_duplicates_and_conflicts() {
        let mut collector = FileFragments::default();
        let fragments = [b"left".as_slice(), b"right".as_slice()]
            .into_iter()
            .enumerate()
            .map(|(index, data)| {
                let mut payload = vec![42; 8];
                payload.extend((index as u16).to_be_bytes());
                payload.extend(2_u16.to_be_bytes());
                payload.push(0x22);
                payload.extend(data);
                Packet::new(FRAGMENT, [1; 8], 123, payload)
            })
            .collect::<Vec<_>>();
        assert!(collector.accept(&fragments[1]).unwrap().is_none());
        assert!(collector.accept(&fragments[1]).unwrap().is_none());
        let mut conflict = fragments[1].clone();
        conflict.payload[13] ^= 1;
        assert!(collector.accept(&conflict).is_err());
        assert!(collector.accept(&fragments[1]).unwrap().is_none());
        assert_eq!(
            collector.accept(&fragments[0]).unwrap().unwrap(),
            b"leftright"
        );
    }
}
