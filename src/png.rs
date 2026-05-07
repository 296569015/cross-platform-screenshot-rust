const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const ADLER_MOD: u32 = 65_521;

pub fn encode_rgba_png(width: i32, height: i32, rgba: &[u8]) -> Option<Vec<u8>> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let row_bytes = width as usize * 4;
    let expected = row_bytes.checked_mul(height as usize)?;
    if rgba.len() != expected {
        return None;
    }

    let mut filtered = Vec::with_capacity((row_bytes + 1) * height as usize);
    for y in 0..height as usize {
        filtered.push(0);
        let row = y * row_bytes;
        filtered.extend_from_slice(&rgba[row..row + row_bytes]);
    }

    let mut png = Vec::with_capacity(filtered.len() + 128);
    png.extend_from_slice(PNG_SIGNATURE);

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.push(8);
    ihdr.push(6);
    ihdr.push(0);
    ihdr.push(0);
    ihdr.push(0);
    write_chunk(&mut png, b"IHDR", &ihdr);
    write_chunk(&mut png, b"IDAT", &zlib_store(&filtered));
    write_chunk(&mut png, b"IEND", &[]);
    Some(png)
}

fn zlib_store(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / 65_535 * 5 + 16);
    out.push(0x78);
    out.push(0x01);

    if data.is_empty() {
        out.push(0x01);
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&(!0_u16).to_le_bytes());
    } else {
        let mut offset = 0;
        while offset < data.len() {
            let remaining = data.len() - offset;
            let len = remaining.min(65_535);
            let final_block = offset + len == data.len();
            out.push(if final_block { 0x01 } else { 0x00 });
            let len16 = len as u16;
            out.extend_from_slice(&len16.to_le_bytes());
            out.extend_from_slice(&(!len16).to_le_bytes());
            out.extend_from_slice(&data[offset..offset + len]);
            offset += len;
        }
    }

    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(kind.len() + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn adler32(data: &[u8]) -> u32 {
    let mut a = 1_u32;
    let mut b = 0_u32;
    for &byte in data {
        a = (a + byte as u32) % ADLER_MOD;
        b = (b + a) % ADLER_MOD;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::encode_rgba_png;

    #[test]
    fn encodes_png_signature_and_chunks() {
        let pixels = vec![255, 0, 0, 255, 0, 255, 0, 255];
        let png = encode_rgba_png(2, 1, &pixels).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert!(png.windows(4).any(|w| w == b"IHDR"));
        assert!(png.windows(4).any(|w| w == b"IDAT"));
        assert!(png.windows(4).any(|w| w == b"IEND"));
    }
}
