//! Decoder for Adobe Animate's `bin/*.dat` embedded bitmap files.
//!
//! Animate stores "imported" bitmaps as one `.dat` per image, referenced from
//! `DOMBitmapItem@bitmapDataHRef`. The layout (mirrors the ActionScript
//! `XFLProcessor`'s `ImageExtractor`) is:
//!
//! * bytes 0..2: type, **big-endian u16**:
//!   `0xffd8` JPEG (the file is a JPEG), `0x0303` 8-bit palette,
//!   `0x0304` 16-bit 555, `0x0305` 32-bit.
//! * bytes 4..6 / 6..8: width / height, **little-endian u16**.
//! * byte 24: transparency flag.
//! * from offset 26: length-prefixed chunks of a **raw deflate** stream,
//!   concatenated then inflated.
//!
//! 32-bit pixels are top-down **ARGB** (byte order `A, R, G, B`).

use std::io::Read;

use super::XflError;

/// A decoded embedded bitmap.
pub enum DatImage {
    /// An encoded image file (JPEG) — load with `Texture2D::from_file_with_format`.
    Encoded(Vec<u8>),
    /// Straight RGBA8 pixels.
    Rgba {
        width: u16,
        height: u16,
        bytes: Vec<u8>,
    },
}

const TYPE_JPEG: u16 = 0xffd8;
const TYPE_8_BITS: u16 = 0x0303;
const TYPE_16_BITS: u16 = 0x0304;
const TYPE_32_BITS: u16 = 0x0305;
const COMPRESSED_OFFSET: usize = 26;

/// Decode a `bin/*.dat` bitmap. Returns `Err` for unsupported types.
pub fn decode(data: &[u8]) -> Result<DatImage, XflError> {
    if data.len() < COMPRESSED_OFFSET + 2 {
        return Err(XflError("dat: file too small".into()));
    }
    let kind = u16::from_be_bytes([data[0], data[1]]);
    match kind {
        TYPE_JPEG => Ok(DatImage::Encoded(data.to_vec())),
        TYPE_8_BITS => decode_8bit(data),
        TYPE_16_BITS => decode_16bit(data),
        TYPE_32_BITS => decode_32bit(data),
        other => Err(XflError(format!("dat: unsupported bitmap type {other:#06x}"))),
    }
}

fn u16_le(data: &[u8], pos: usize) -> Result<usize, XflError> {
    let a = *data
        .get(pos)
        .ok_or_else(|| XflError("dat: truncated header".into()))?;
    let b = *data
        .get(pos + 1)
        .ok_or_else(|| XflError("dat: truncated header".into()))?;
    Ok(u16::from_le_bytes([a, b]) as usize)
}

fn size(data: &[u8]) -> Result<(u16, u16), XflError> {
    let w = u16_le(data, 4)? as u16;
    let h = u16_le(data, 6)? as u16;
    Ok((w, h))
}

/// Concatenate the length-prefixed chunks starting at `pos`, then raw-inflate.
fn decompress_chunks(data: &[u8], pos: usize) -> Result<Vec<u8>, XflError> {
    let mut pos = pos;
    let mut len = u16_le(data, pos)?;
    pos += 2;
    pos += 2; // skip the two bytes after the first length field
    if len == 2 {
        len = u16_le(data, pos)?;
        pos += 2;
    } else {
        len = len.saturating_sub(2);
    }

    let mut compressed = Vec::new();
    while len > 0 {
        let end = (pos + len).min(data.len());
        compressed.extend_from_slice(&data[pos..end]);
        pos += len;
        if pos + 2 > data.len() {
            break;
        }
        len = u16_le(data, pos)?;
        pos += 2;
    }

    let mut out = Vec::new();
    flate2::read::DeflateDecoder::new(&compressed[..])
        .read_to_end(&mut out)
        .map_err(|e| XflError(format!("dat: inflate failed: {e}")))?;
    Ok(out)
}

fn decode_32bit(data: &[u8]) -> Result<DatImage, XflError> {
    let (width, height) = size(data)?;
    let raw = decompress_chunks(data, COMPRESSED_OFFSET)?;
    let pixels = width as usize * height as usize;
    if raw.len() < pixels * 4 {
        return Err(XflError(format!(
            "dat: 32-bit payload too short ({} < {})",
            raw.len(),
            pixels * 4
        )));
    }
    // ARGB (A,R,G,B) -> RGBA.
    let mut bytes = Vec::with_capacity(pixels * 4);
    for px in raw[..pixels * 4].chunks_exact(4) {
        bytes.extend_from_slice(&[px[1], px[2], px[3], px[0]]);
    }
    Ok(DatImage::Rgba {
        width,
        height,
        bytes,
    })
}

fn decode_16bit(data: &[u8]) -> Result<DatImage, XflError> {
    let (width, height) = size(data)?;
    let raw = decompress_chunks(data, COMPRESSED_OFFSET)?;
    let pixels = width as usize * height as usize;
    let mut bytes = Vec::with_capacity(pixels * 4);
    for px in raw[..(pixels * 2).min(raw.len())].chunks_exact(2) {
        let v = u16::from_le_bytes([px[0], px[1]]);
        let r = ((v >> 10) & 0x1f) as u32;
        let g = ((v >> 5) & 0x1f) as u32;
        let b = (v & 0x1f) as u32;
        let expand = |c: u32| ((c << 3 | c >> 2) & 0xff) as u8;
        bytes.extend_from_slice(&[expand(r), expand(g), expand(b), 255]);
    }
    bytes.resize(pixels * 4, 0);
    Ok(DatImage::Rgba {
        width,
        height,
        bytes,
    })
}

fn decode_8bit(data: &[u8]) -> Result<DatImage, XflError> {
    let (width, height) = size(data)?;
    // Palette starts right after the 1-byte transparency flag at offset 24.
    let mut pos = 25;
    let palette_len = u16_le(data, pos)?;
    pos += 2;
    let mut palette = Vec::with_capacity(palette_len);
    for _ in 0..palette_len {
        let v = data
            .get(pos..pos + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| XflError("dat: truncated palette".into()))?;
        palette.push([
            (v >> 16) as u8,
            (v >> 8) as u8,
            v as u8,
            (v >> 24) as u8,
        ]);
        pos += 4;
    }
    let indices = decompress_chunks(data, pos + 1)?;

    let w = width as usize;
    let stride = (w + 3) / 4 * 4;
    let mut bytes = Vec::with_capacity(w * height as usize * 4);
    for row in 0..height as usize {
        for col in 0..w {
            let idx = indices.get(row * stride + col).copied().unwrap_or(0) as usize;
            bytes.extend_from_slice(palette.get(idx).copied().unwrap_or([0, 0, 0, 0]).as_slice());
        }
    }
    Ok(DatImage::Rgba {
        width,
        height,
        bytes,
    })
}
