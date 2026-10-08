//! Bounded public-text compression, following pinned Unlicensed BitFoundation.
//! Raw DEFLATE bytes can differ between encoders; stock-phone signing
//! compatibility must be checked on hardware, not inferred from round trips.
use flate2::{write::DeflateEncoder, Compression, Decompress, FlushDecompress, Status};
use std::io::Write;

pub const MAX_TEXT_BYTES: usize = 1024;
const COMPRESSION_THRESHOLD: usize = 100;
const MAX_FRAGMENT_BYTES: usize = 1024;

fn length_width(version: u8) -> Result<usize, &'static str> {
    match version {
        1 => Ok(2),
        2 => Ok(4),
        _ => Err("unsupported text compression version"),
    }
}

/// Match Swift's threshold, whole-payload byte-diversity gate and requirement
/// that raw DEFLATE alone be smaller than the source (before the size prefix).
pub fn compress_text(payload: &[u8], version: u8) -> Result<Option<Vec<u8>>, &'static str> {
    let width = length_width(version)?;
    if payload.len() > MAX_TEXT_BYTES {
        return Err("public text exceeds 1024-byte budget");
    }
    if payload.len() < COMPRESSION_THRESHOLD {
        return Ok(None);
    }
    let mut unique = [false; 256];
    for byte in payload {
        unique[*byte as usize] = true;
    }
    if unique.iter().filter(|present| **present).count() * 10 >= payload.len().min(256) * 9 {
        return Ok(None);
    }
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::new(6));
    encoder
        .write_all(payload)
        .map_err(|_| "text compression failed")?;
    let compressed = encoder.finish().map_err(|_| "text compression failed")?;
    if compressed.len() >= payload.len() {
        return Ok(None);
    }
    let mut wire = if width == 2 {
        (payload.len() as u16).to_be_bytes().to_vec()
    } else {
        (payload.len() as u32).to_be_bytes().to_vec()
    };
    wire.extend(compressed);
    Ok(Some(wire))
}

/// Check sizes before verification or allocation. This does not expand bytes.
pub fn original_size(wire: &[u8], version: u8) -> Result<usize, &'static str> {
    let width = length_width(version)?;
    if wire.len() <= width || wire.len() > MAX_TEXT_BYTES + width {
        return Err("invalid compressed text length");
    }
    let size = if width == 2 {
        u16::from_be_bytes(wire[..2].try_into().unwrap()) as usize
    } else {
        u32::from_be_bytes(wire[..4].try_into().unwrap()) as usize
    };
    if size == 0 || size > MAX_TEXT_BYTES || size > (wire.len() - width).saturating_mul(1032) {
        return Err("invalid compressed text bounds");
    }
    Ok(size)
}

/// Caller verifies the complete signed public message before inflating it.
pub fn inflate_text(wire: &[u8], version: u8) -> Result<Vec<u8>, &'static str> {
    let expected = original_size(wire, version)?;
    inflate_exact(&wire[length_width(version)?..], expected)
}

/// Unsigned fragment contents remain untrusted. Only a bounded text-marked
/// prefix may enter the text collector; the final outer signature is required.
pub fn inflate_text_fragment(wire: &[u8], version: u8) -> Result<Vec<u8>, &'static str> {
    let expected = original_size(wire, version)?;
    if !(14..=MAX_FRAGMENT_BYTES).contains(&expected) {
        return Err("compressed text fragment exceeds bounds");
    }
    let payload = inflate_exact(&wire[length_width(version)?..], expected)?;
    if payload[12] != 0x02 {
        return Err("compressed fragment is not public text");
    }
    Ok(payload)
}

fn inflate_exact(compressed: &[u8], expected: usize) -> Result<Vec<u8>, &'static str> {
    let mut output = vec![0; expected + 1];
    let mut inflater = Decompress::new(false);
    loop {
        let (before_in, before_out) = (inflater.total_in(), inflater.total_out());
        let status = inflater
            .decompress(
                &compressed[before_in as usize..],
                &mut output[before_out as usize..],
                FlushDecompress::Finish,
            )
            .map_err(|_| "invalid text raw DEFLATE stream")?;
        let (read, written) = (inflater.total_in() as usize, inflater.total_out() as usize);
        if written > expected {
            return Err("expanded text exceeds declared size");
        }
        if status == Status::StreamEnd {
            if read != compressed.len() || written != expected {
                return Err("incomplete or trailing text raw DEFLATE data");
            }
            output.truncate(expected);
            return Ok(output);
        }
        if read == before_in as usize && written == before_out as usize {
            return Err("stalled or incomplete text raw DEFLATE stream");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_diversity_and_decoded_budget() {
        assert!(compress_text(&[b'a'; 99], 1).unwrap().is_none());
        for version in [1, 2] {
            let wire = compress_text(&[b'a'; 100], version).unwrap().unwrap();
            assert_eq!(inflate_text(&wire, version).unwrap(), [b'a'; 100]);
            let maximum = "🙂".repeat(256).into_bytes();
            let wire = compress_text(&maximum, version).unwrap().unwrap();
            assert_eq!(original_size(&wire, version).unwrap(), MAX_TEXT_BYTES);
            assert_eq!(inflate_text(&wire, version).unwrap(), maximum);
        }
        let diverse: Vec<u8> = (0..=255).cycle().take(1024).collect();
        assert!(compress_text(&diverse, 1).unwrap().is_none());
        assert!(compress_text(&[b'a'; 1025], 1).is_err());
    }

    #[test]
    fn rejects_overflow_trailing_truncated_and_mismatched_streams() {
        let wire = compress_text(&[b'a'; 1024], 1).unwrap().unwrap();
        for size in [0_u16, 1023, 1025, u16::MAX] {
            let mut bad = wire.clone();
            bad[..2].copy_from_slice(&size.to_be_bytes());
            assert!(inflate_text(&bad, 1).is_err());
        }
        let mut bad = wire.clone();
        bad.push(0);
        assert!(inflate_text(&bad, 1).is_err());
        for end in 0..wire.len() {
            assert!(inflate_text(&wire[..end], 1).is_err());
        }
        assert!(inflate_text(&[0, 1, 255], 1).is_err());
        assert!(inflate_text(&wire, 3).is_err());
    }
}
