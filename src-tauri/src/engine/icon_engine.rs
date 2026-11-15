use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage, Rgba};
use std::collections::HashMap;
use std::io::Cursor;

pub struct IconEngine;

impl IconEngine {
    pub async fn fetch_and_generate_icons(
        favicon_url: Option<&str>,
        fallback_name: &str,
    ) -> Result<(HashMap<String, Vec<u8>>, String), String> {
        let mut raw_bytes: Option<Vec<u8>> = None;

        if let Some(url) = favicon_url {
            let client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .build()
                .unwrap_or_default();

            if let Ok(resp) = client.get(url).send().await {
                if resp.status().is_success() {
                    if let Ok(bytes) = resp.bytes().await {
                        if !bytes.is_empty() {
                            raw_bytes = Some(bytes.to_vec());
                        }
                    }
                }
            }
        }

        let loaded = raw_bytes
            .as_deref()
            .and_then(|bytes| image::load_from_memory(bytes).ok());
        let used_fallback = loaded.is_none();
        let img = loaded.unwrap_or_else(|| Self::generate_fallback_image(fallback_name));

        let mut mipmaps = HashMap::new();
        let sizes = [
            ("mipmap-hdpi", 72),
            ("mipmap-xhdpi", 96),
            ("mipmap-xxhdpi", 144),
            ("mipmap-xxxhdpi", 192),
        ];

        let mut base64_icon = String::new();

        for (dir_name, size) in sizes {
            let resized = img.resize_exact(size, size, FilterType::Lanczos3);
            let mut buf = Cursor::new(Vec::new());
            resized.write_to(&mut buf, image::ImageFormat::Png)
                .map_err(|e| format!("Error codificando PNG: {}", e))?;

            let bytes = buf.into_inner();
            if size == 192 {
                base64_icon = if used_fallback {
                    Self::generate_fallback_icon_uri(fallback_name)
                } else {
                    format!("data:image/png;base64,{}", base64_encode(&bytes))
                };
            }
            mipmaps.insert(dir_name.to_string(), bytes);
        }

        Ok((mipmaps, base64_icon))
    }

    fn generate_fallback_image(name: &str) -> DynamicImage {
        // Genera una imagen cuadrada de 192x192 con gradiente estable por nombre.
        let mut img = RgbaImage::new(192, 192);
        let (base_r, base_g, base_b) = Self::color_for_name(name);

        for y in 0..192 {
            for x in 0..192 {
                let r = base_r.saturating_add(((x * 40) / 192) as u8);
                let g = base_g.saturating_add(((y * 32) / 192) as u8);
                let b = base_b.saturating_add((((x + y) * 24) / 384) as u8);
                img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }

        DynamicImage::ImageRgba8(img)
    }

    /// Icono vectorial de respaldo con la inicial del nombre. Se usa en el
    /// catálogo cuando el sitio no expone un favicon descargable.
    fn generate_fallback_icon_uri(name: &str) -> String {
        let letter = name
            .chars()
            .find(|c| c.is_ascii_alphanumeric())
            .unwrap_or('A')
            .to_ascii_uppercase();
        let (r, g, b) = Self::color_for_name(name);

        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 192 192\"><defs><linearGradient id=\"g\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\"><stop stop-color=\"#{:02X}{:02X}{:02X}\"/><stop offset=\"1\" stop-color=\"#7C3AED\"/></linearGradient></defs><rect width=\"192\" height=\"192\" rx=\"40\" fill=\"url(#g)\"/><text x=\"96\" y=\"130\" text-anchor=\"middle\" font-family=\"Segoe UI,Arial,sans-serif\" font-size=\"104\" font-weight=\"700\" fill=\"#FFFFFF\">{}</text></svg>",
            r, g, b, letter
        );

        format!("data:image/svg+xml;utf8,{}", percent_encode(&svg))
    }

    fn color_for_name(name: &str) -> (u8, u8, u8) {
        let mut hash: u32 = 2_166_136_261;
        for c in name.chars() {
            hash ^= c as u32;
            hash = hash.wrapping_mul(16_777_619);
        }
        let r = 70 + ((hash >> 2) & 0x3F) as u8;
        let g = 40 + ((hash >> 9) & 0x3F) as u8;
        let b = 130 + ((hash >> 16) & 0x5F) as u8;
        (r, g, b)
    }
}

fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

fn base64_encode(data: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        result.push(CHARSET[(b0 >> 2) as usize] as char);
        result.push(CHARSET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARSET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARSET[(b2 & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
