use super::Invitation;

/// QR modules including the four-module white quiet zone.
pub fn modules(value: &str) -> Result<(usize, Vec<bool>), String> {
    let code = qrcode::QrCode::new(value).map_err(|e| e.to_string())?;
    let side = code.width() + 8;
    let mut pixels = vec![false; side * side];
    for y in 0..code.width() {
        for x in 0..code.width() {
            pixels[(y + 4) * side + x + 4] = code[(x, y)] == qrcode::Color::Dark;
        }
    }
    Ok((side, pixels))
}
pub fn decode(width: usize, height: usize, gray: &[u8]) -> Result<Invitation, String> {
    if width == 0 || height == 0 || width > 4096 || height > 4096 || gray.len() != width * height {
        return Err("二维码图像尺寸无效".into());
    }
    let mut decoder = quircs::Quirc::default();
    for code in decoder.identify(width, height, gray) {
        if let Ok(data) = code
            .map_err(|e| e.to_string())
            .and_then(|c| c.decode().map_err(|e| e.to_string()))
        {
            if let Ok(text) = std::str::from_utf8(&data.payload) {
                if let Ok(invitation) = Invitation::parse(text) {
                    return Ok(invitation);
                }
            }
        }
    }
    Err("未识别到 LambdaDX 连接二维码".into())
}
pub fn from_image(path: &std::path::Path) -> Result<Invitation, String> {
    let reader = image::io::Reader::open(path).map_err(|e| e.to_string())?;
    let mut reader = reader.with_guessed_format().map_err(|e| e.to_string())?;
    let mut limits = image::io::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    reader.limits(limits);
    let gray = reader.decode().map_err(|e| e.to_string())?.to_luma8();
    decode(gray.width() as usize, gray.height() as usize, &gray)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_pairing_qr_can_be_scanned() {
        let invitation = Invitation {
            address: "192.168.1.23:47653".into(),
            token: "0123456789abcdef0123456789abcdef".into(),
        };
        let (side, cells) = modules(&invitation.encode()).unwrap();
        let width = side * 6;
        let pixels: Vec<_> = (0..width * width)
            .map(|i| {
                if cells[(i / width / 6) * side + i % width / 6] {
                    0
                } else {
                    255
                }
            })
            .collect();
        assert_eq!(
            decode(width, width, &pixels).unwrap().encode(),
            invitation.encode()
        );
    }
}
