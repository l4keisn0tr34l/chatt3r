//! The canonical BitChat v2 file TLV payload, not a new transfer protocol.
//! This is a bounded pure codec, not a BLE sender or a disk-saving receiver.
//! Based on the Unlicensed `BitchatFilePacket.swift` at the pinned upstream revision.

/// A deliberately small parser budget, NOT the upstream 1 MiB ceiling or a
/// proven BLE transfer limit. At a 128-byte frame limit, even a 64 KiB file
/// would exceed the deployed Android 256-fragment receive cap. The current
/// `chatt3r` binary still rejects file traffic altogether.
pub const MAX_FILE_BYTES: usize = 64 * 1024;
const MAX_ENVELOPE_BYTES: usize = MAX_FILE_BYTES + 1024;
const MAX_METADATA_BYTES: usize = 255;

#[derive(Debug, PartialEq, Eq)]
pub struct FilePayload {
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub content: Vec<u8>,
}

fn next<'a>(bytes: &'a [u8], cursor: &mut usize, len: usize) -> Result<&'a [u8], &'static str> {
    let end = cursor.checked_add(len).ok_or("file TLV length overflow")?;
    let value = bytes.get(*cursor..end).ok_or("truncated file TLV")?;
    *cursor = end;
    Ok(value)
}

impl FilePayload {
    /// A deliberately strict subset of canonical v2: name/mime use BE u16
    /// lengths; size uses BE u16 length=4
    /// followed by a BE u32; content uses a BE u32 length. Unknown tags have
    /// BE u16 lengths and are ignored. Optional metadata is never a disk path.
    /// Unlike the upstream tolerant reader, legacy lengths and repeated
    /// content TLVs are rejected until interoperability tests justify them.
    pub fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() > MAX_ENVELOPE_BYTES {
            return Err("file envelope exceeds first-transfer budget");
        }
        let mut cursor = 0;
        let (mut name, mut mime, mut size, mut content) = (None, None, None, None);
        while cursor < bytes.len() {
            let tag = next(bytes, &mut cursor, 1)?[0];
            let length = if tag == 4 {
                u32::from_be_bytes(next(bytes, &mut cursor, 4)?.try_into().unwrap()) as usize
            } else {
                u16::from_be_bytes(next(bytes, &mut cursor, 2)?.try_into().unwrap()) as usize
            };
            let value = next(bytes, &mut cursor, length)?;
            match tag {
                1 | 3 => {
                    if value.len() > MAX_METADATA_BYTES {
                        return Err("file metadata exceeds first-transfer budget");
                    }
                    let text =
                        std::str::from_utf8(value).map_err(|_| "invalid file metadata UTF-8")?;
                    let target = if tag == 1 { &mut name } else { &mut mime };
                    if target.replace(text.to_owned()).is_some() {
                        return Err("duplicate file metadata");
                    }
                }
                2 => {
                    if value.len() != 4
                        || size
                            .replace(u32::from_be_bytes(value.try_into().unwrap()))
                            .is_some()
                    {
                        return Err("invalid or duplicate file size");
                    }
                }
                4 => {
                    if value.is_empty() || value.len() > MAX_FILE_BYTES || content.is_some() {
                        return Err("invalid or duplicate file content");
                    }
                    content = Some(value.to_vec());
                }
                _ => {} // Current upstream ignores unknown optional TLVs.
            }
        }
        let content: Vec<u8> = content.ok_or("missing file content")?;
        if size.ok_or("missing declared file size")? as usize != content.len() {
            return Err("declared file size does not match content");
        }
        Ok(Self {
            file_name: name,
            mime_type: mime,
            content,
        })
    }

    /// Create current stock BitChat's file TLV layout. This does not sign or
    /// fragment the enclosing v2 type-0x22 packet; the chat binary cannot yet
    /// safely do that for long/compressed payloads.
    pub fn encode(&self) -> Result<Vec<u8>, &'static str> {
        if self.content.is_empty() || self.content.len() > MAX_FILE_BYTES {
            return Err("file content exceeds first-transfer budget");
        }
        let mut bytes = Vec::with_capacity(self.content.len() + 64);
        if let Some(name) = &self.file_name {
            append_text(&mut bytes, 1, name)?;
        }
        append_size(&mut bytes, self.content.len() as u32);
        if let Some(mime) = &self.mime_type {
            append_text(&mut bytes, 3, mime)?;
        }
        bytes.push(4);
        bytes.extend((self.content.len() as u32).to_be_bytes());
        bytes.extend(&self.content);
        Ok(bytes)
    }
}

fn append_text(bytes: &mut Vec<u8>, tag: u8, text: &str) -> Result<(), &'static str> {
    if text.len() > MAX_METADATA_BYTES {
        return Err("file metadata exceeds first-transfer budget");
    }
    bytes.push(tag);
    bytes.extend((text.len() as u16).to_be_bytes());
    bytes.extend(text.as_bytes());
    Ok(())
}

fn append_size(bytes: &mut Vec<u8>, size: u32) {
    bytes.push(2);
    bytes.extend(4_u16.to_be_bytes());
    bytes.extend(size.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (serde_json::Value, Vec<u8>) {
        let vector: serde_json::Value =
            serde_json::from_str(include_str!("../../test-vectors/file-payload-v2.json")).unwrap();
        let bytes = hex::decode(vector["payload_hex"].as_str().unwrap()).unwrap();
        (vector, bytes)
    }

    #[test]
    fn independent_canonical_v2_layout_and_roundtrip() {
        let (vector, bytes) = fixture();
        let decoded = FilePayload::decode(&bytes).unwrap();
        assert_eq!(decoded.file_name.as_deref(), vector["file_name"].as_str());
        assert_eq!(decoded.mime_type.as_deref(), vector["mime_type"].as_str());
        assert_eq!(
            hex::encode(&decoded.content),
            vector["content_hex"].as_str().unwrap()
        );
        assert_eq!(decoded.encode().unwrap(), bytes);
        assert_eq!(vector["outer_type"], 0x22);
        assert_eq!(vector["outer_version"], 2);
    }

    #[test]
    fn bounded_and_untrusted_tlvs() {
        let (_, good) = fixture();
        for cut in 0..good.len() {
            assert!(FilePayload::decode(&good[..cut]).is_err());
        }
        let mut wrong_size = good.clone();
        let pos = good
            .windows(7)
            .position(|window| window.starts_with(&[2, 0, 4]))
            .unwrap();
        wrong_size[pos + 6] ^= 1;
        assert!(FilePayload::decode(&wrong_size).is_err());
        let mut unknown = vec![0x77, 0, 2, 0xbe, 0xef];
        unknown.extend(&good);
        assert_eq!(
            FilePayload::decode(&unknown).unwrap().encode().unwrap(),
            good
        );
        let mut duplicate = good.clone();
        duplicate.extend([2, 0, 4, 0, 0, 0, 1]);
        assert!(FilePayload::decode(&duplicate).is_err());
        let mut huge = good.clone();
        let content_at = good
            .windows(5)
            .position(|window| window[0] == 4 && window[1..] == [0, 0, 0, 18])
            .unwrap();
        huge[content_at + 1..content_at + 5].fill(0xff);
        assert!(FilePayload::decode(&huge).is_err());
        assert!(FilePayload::decode(&vec![0; MAX_ENVELOPE_BYTES + 1]).is_err());
    }

    #[test]
    fn malformed_input_never_panics_or_allocates_from_claimed_length() {
        for length in 0..1024 {
            let bytes: Vec<u8> = (0..length).map(|_| rand::random()).collect();
            let _ = FilePayload::decode(&bytes);
        }
        let (_, mut bytes) = fixture();
        bytes[3] = 0xff; // absurd filename length, only 68 bytes actually present
        assert!(FilePayload::decode(&bytes).is_err());
        let boundary = FilePayload {
            file_name: None,
            mime_type: None,
            content: vec![0xaa; MAX_FILE_BYTES],
        };
        assert_eq!(
            FilePayload::decode(&boundary.encode().unwrap()).unwrap(),
            boundary
        );
        assert!(FilePayload {
            file_name: None,
            mime_type: None,
            content: vec![0xaa; MAX_FILE_BYTES + 1],
        }
        .encode()
        .is_err());
    }

    #[test]
    fn metadata_remains_untrusted_and_no_disk_io_occurs() {
        let file = FilePayload {
            file_name: Some("../../outside.txt".into()),
            mime_type: None,
            content: vec![1],
        };
        let decoded = FilePayload::decode(&file.encode().unwrap()).unwrap();
        assert_eq!(decoded.file_name.as_deref(), Some("../../outside.txt"));
        // A future receive policy must sanitize/reject the name before using
        // it on disk; the wire codec deliberately cannot authorize paths.
        assert!(FilePayload::decode(&[4, 0, 0, 0, 1, 0]).is_err());
    }
}
