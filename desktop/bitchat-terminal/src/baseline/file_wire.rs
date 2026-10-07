//! Software-only v2 public file outer-frame check. No BLE, decompression, or saving.
//! Layout/padding follows the pinned Unlicensed BitFoundation BinaryProtocol.swift.
//! The compressed preimage authenticates the *wire bytes*; it does not prove
//! that another implementation would recompress to identical canonical bytes.

use ed25519_dalek::{Signature, VerifyingKey};
use flate2::{Decompress, FlushDecompress, Status};

use super::file_packet::{FilePayload, MAX_FILE_BYTES};

const MAX_TLV_BYTES: usize = MAX_FILE_BYTES + 1024;
const HEADER: usize = 24; // v2 header (16) + sender (8); broadcast has no recipient
const MAX_WIRE_BYTES: usize = HEADER + MAX_TLV_BYTES + 64;

/// A parsed, still-untrusted frame. The caller must obtain the sender's key
/// from a separately verified announcement and call `verify_wire_signature`.
#[derive(Debug)]
pub struct FileWire<'a> {
    pub sender: [u8; 8],
    pub timestamp: u64,
    pub ttl: u8,
    pub original_size: Option<usize>,
    pub wire_payload: &'a [u8], // raw DEFLATE if original_size is Some
    signature: [u8; 64],
}

impl<'a> FileWire<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, &'static str> {
        if bytes.len() < HEADER + 64 || bytes.len() > MAX_WIRE_BYTES {
            return Err("invalid file frame length");
        }
        // Only signed v2 public file broadcasts. Reject recipient, route,
        // RSR and reserved flags instead of silently losing their context.
        let flags = bytes[11];
        if bytes[0] != 2 || bytes[1] != 0x22 || flags & !0x06 != 0 || flags & 0x02 == 0 {
            return Err("not a supported signed public v2 file frame");
        }
        let len = u32::from_be_bytes(bytes[12..16].try_into().unwrap()) as usize;
        if len > MAX_TLV_BYTES
            || HEADER.checked_add(len).and_then(|n| n.checked_add(64)) != Some(bytes.len())
        {
            return Err("invalid file payload length or trailing bytes");
        }
        let (original_size, wire_payload) = if flags & 0x04 != 0 {
            if len < 5 {
                return Err("missing original size or deflate bytes");
            }
            let size = u32::from_be_bytes(bytes[HEADER..HEADER + 4].try_into().unwrap()) as usize;
            let deflate = &bytes[HEADER + 4..HEADER + len];
            if size == 0
                || size > MAX_TLV_BYTES
                || deflate.is_empty()
                || size > deflate.len().saturating_mul(1032)
            {
                return Err("invalid compressed file bounds");
            }
            (Some(size), deflate)
        } else {
            (None, &bytes[HEADER..HEADER + len])
        };
        Ok(Self {
            sender: bytes[16..HEADER].try_into().unwrap(),
            timestamp: u64::from_be_bytes(bytes[3..11].try_into().unwrap()),
            ttl: bytes[2],
            original_size,
            wire_payload,
            signature: bytes[HEADER + len..].try_into().unwrap(),
        })
    }

    /// TTL changes on relay. Rebuild the unsigned default-padded preimage
    /// over the original wire payload; never accept this alone as a file.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let len = self.wire_payload.len() + if self.original_size.is_some() { 4 } else { 0 };
        let mut bytes = Vec::with_capacity(HEADER + len + 255);
        bytes.extend([2, 0x22, 0]);
        bytes.extend(self.timestamp.to_be_bytes());
        bytes.push(if self.original_size.is_some() {
            0x04
        } else {
            0
        });
        bytes.extend((len as u32).to_be_bytes());
        bytes.extend(self.sender);
        if let Some(size) = self.original_size {
            bytes.extend((size as u32).to_be_bytes());
        }
        bytes.extend(self.wire_payload);
        // BitFoundation MessagePadding.optimalBlockSize accounts for 16 bytes
        // of overhead; pad declines buckets requiring >255 pad bytes.
        if let Some(bucket) = [256, 512, 1024, 2048]
            .into_iter()
            .find(|n| bytes.len() + 16 <= *n)
        {
            let pad = bucket - bytes.len();
            if pad <= 255 {
                bytes.resize(bucket, pad as u8);
            }
        }
        bytes
    }

    pub fn verify_wire_signature(&self, key: &VerifyingKey) -> Result<(), &'static str> {
        key.verify_strict(
            &self.signing_bytes(),
            &Signature::from_bytes(&self.signature),
        )
        .map_err(|_| "invalid file wire signature")
    }

    /// Only the uncompressed case can be passed to the bounded TLV decoder.
    /// Call *after* signature verification; this method is not authentication.
    pub fn uncompressed_payload(&self) -> Result<FilePayload, &'static str> {
        if self.original_size.is_some() {
            return Err("compressed file requires a bounded raw DEFLATE decoder");
        }
        FilePayload::decode(self.wire_payload)
    }

    /// Verify before expanding untrusted bytes. The caller must first bind
    /// sender/timestamp to its live peer and a verified announcement key.
    pub fn decode_verified(&self, key: &VerifyingKey) -> Result<FilePayload, &'static str> {
        self.verify_wire_signature(key)?;
        match self.original_size {
            None => self.uncompressed_payload(),
            Some(size) => {
                let bytes = inflate_exact(self.wire_payload, size)?;
                FilePayload::decode(&bytes)
            }
        }
    }
}

fn inflate_exact(compressed: &[u8], expected: usize) -> Result<Vec<u8>, &'static str> {
    if expected == 0 || expected > MAX_TLV_BYTES || compressed.is_empty() {
        return Err("invalid raw DEFLATE bounds");
    }
    // One extra output byte detects under-declared expansion. Require the
    // end marker AND full input consumption; a full output buffer is not proof
    // of completion, and trailing compressed bytes must not be accepted.
    let mut output = vec![0; expected + 1];
    let mut inflater = Decompress::new(false); // false = raw DEFLATE, no zlib header
    loop {
        let (before_in, before_out) = (inflater.total_in(), inflater.total_out());
        let status = inflater
            .decompress(
                &compressed[before_in as usize..],
                &mut output[before_out as usize..],
                FlushDecompress::Finish,
            )
            .map_err(|_| "invalid raw DEFLATE stream")?;
        let (read, written) = (inflater.total_in() as usize, inflater.total_out() as usize);
        if written > expected {
            return Err("expanded file exceeds declared size");
        }
        if status == Status::StreamEnd {
            if read != compressed.len() || written != expected {
                return Err("incomplete or trailing raw DEFLATE data");
            }
            output.truncate(expected);
            return Ok(output);
        }
        if read == before_in as usize && written == before_out as usize {
            return Err("stalled or incomplete raw DEFLATE stream");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vectors() -> serde_json::Value {
        serde_json::from_str(include_str!("../../test-vectors/file-wire-v2.json")).unwrap()
    }

    #[test]
    fn independent_signed_v2_preimages() {
        let vectors = vectors();
        let public_key: [u8; 32] = hex::decode(vectors["public_key_hex"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let key = VerifyingKey::from_bytes(&public_key).unwrap();
        for case in vectors["cases"].as_array().unwrap() {
            let bytes = hex::decode(case["wire_hex"].as_str().unwrap()).unwrap();
            let parsed = FileWire::parse(&bytes).unwrap();
            let mut expected =
                hex::decode(case["signing_input_prefix_hex"].as_str().unwrap()).unwrap();
            expected.extend(std::iter::repeat_n(
                case["signing_padding_byte"].as_u64().unwrap() as u8,
                case["signing_padding_count"].as_u64().unwrap() as usize,
            ));
            assert_eq!(parsed.signing_bytes(), expected);
            assert_eq!(
                parsed.original_size.is_some(),
                case["compressed"].as_bool().unwrap()
            );
            parsed.verify_wire_signature(&key).unwrap();
            let decoded = parsed.decode_verified(&key).unwrap();
            assert_eq!(decoded.file_name.as_deref(), Some("note.txt"));
            if parsed.original_size.is_none() {
                let file = parsed.uncompressed_payload().unwrap();
                assert_eq!(file.file_name.as_deref(), Some("note.txt"));
                assert_eq!(file.content, b"hello from nearby\n");
            } else {
                assert!(parsed.uncompressed_payload().is_err());
                assert_eq!(decoded.content, vec![b'A'; 400]);
                assert_eq!(
                    parsed.original_size,
                    Some(case["payload_length"].as_u64().unwrap() as usize)
                );
                assert_eq!(
                    hex::encode(parsed.wire_payload),
                    case["raw_deflate_hex"].as_str().unwrap()
                );
            }
            let mut changed_ttl = bytes.clone();
            changed_ttl[2] = 1;
            FileWire::parse(&changed_ttl)
                .unwrap()
                .verify_wire_signature(&key)
                .unwrap();
            let mut tampered = bytes.clone();
            tampered[HEADER + 4] ^= 1;
            assert!(FileWire::parse(&tampered)
                .unwrap()
                .verify_wire_signature(&key)
                .is_err());
            let last = tampered.len() - 1;
            tampered = bytes;
            tampered[last] ^= 1;
            assert!(FileWire::parse(&tampered)
                .unwrap()
                .verify_wire_signature(&key)
                .is_err());
        }
    }

    #[test]
    fn raw_deflate_must_finish_exactly_at_declared_size_and_input_end() {
        let data = vectors();
        let case = &data["cases"][1];
        let compressed = hex::decode(case["raw_deflate_hex"].as_str().unwrap()).unwrap();
        assert_eq!(inflate_exact(&compressed, 450).unwrap().len(), 450);
        assert!(inflate_exact(&compressed, 449).is_err());
        assert!(inflate_exact(&compressed, 451).is_err());
        assert!(inflate_exact(&compressed[..compressed.len() - 1], 450).is_err());
        let mut trailing = compressed.clone();
        trailing.push(0);
        assert!(inflate_exact(&trailing, 450).is_err());
        assert!(inflate_exact(&[0xff, 0xff, 0xff], 450).is_err());
        assert!(inflate_exact(&compressed, MAX_TLV_BYTES + 1).is_err());
    }

    #[test]
    fn fail_closed_on_bad_flags_lengths_and_unsigned_files() {
        let bytes = hex::decode(vectors()["cases"][0]["wire_hex"].as_str().unwrap()).unwrap();
        for end in 0..bytes.len() {
            assert!(FileWire::parse(&bytes[..end]).is_err());
        }
        for flag in [0, 1, 8, 0x12, 0x82] {
            let mut changed = bytes.clone();
            changed[11] = flag;
            assert!(FileWire::parse(&changed).is_err());
        }
        let mut bad = bytes.clone();
        bad[15] = 0xff;
        assert!(FileWire::parse(&bad).is_err());
        let mut bad = bytes.clone();
        bad.push(0);
        assert!(FileWire::parse(&bad).is_err());
        let mut bad = bytes;
        bad[11] |= 4;
        bad[HEADER..HEADER + 4].fill(0xff);
        assert!(FileWire::parse(&bad).is_err());
        for len in 0..5_u32 {
            let mut short = bad[..HEADER].to_vec();
            short[12..16].copy_from_slice(&len.to_be_bytes());
            short.extend(vec![0; len as usize + 64]);
            assert!(FileWire::parse(&short).is_err());
        }
        for len in 0..512 {
            let bytes: Vec<u8> = (0..len).map(|_| rand::random()).collect();
            let _ = FileWire::parse(&bytes);
        }
    }
}
