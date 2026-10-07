//! First-test file-only whole-packet 0x20 collector. Never passes unsigned
//! fragment contents to the store; caller validates the reassembled outer frame.
use super::protocol::{Packet, FRAGMENT};
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const MAX_FILE_FRAME: usize = 16 * 1024; // conservative first-radio budget
const MAX_PENDING: usize = 8;

struct Assembly {
    started: Instant,
    timestamp: u64,
    parts: Vec<Option<Vec<u8>>>,
    size: usize,
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
            return Err("file frame exceeds first-radio budget");
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
