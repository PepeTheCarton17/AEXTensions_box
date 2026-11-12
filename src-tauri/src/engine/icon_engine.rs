use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, RgbaImage, Rgba};
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

        let img = match raw_bytes {
            Some(bytes) => {
                image::load_from_memory(&bytes).unwrap_or_else(|_| Self::generate_fallback_image(fallback_name))
            }
            None => Self::generate_fallback_image(fallback_name),
        };

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
                base64_icon = format!("data:image/png;base64,{}", base64_encode(&bytes));
            }
            mipmaps.insert(dir_name.to_string(), bytes);
        }

        Ok((mipmaps, base64_icon))
    }

    fn generate_fallback_image(name: &str) -> DynamicImage {
        // Genera una imagen limpia cuadrada de 192x192 con gradiente y la inicial
        let mut img = RgbaImage::new(192, 192);
        let _initial = name.chars().next().unwrap_or('A').to_ascii_uppercase();
        
        // Colores temáticos Nekoyomi (púrpura oscuro a violeta)
        for y in 0..192 {
            for x in 0..192 {
                let r = (90 + (x * 50) / 192) as u8;
                let g = (40 + (y * 40) / 192) as u8;
                let b = (160 + ((x + y) * 30) / 384) as u8;
                img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }

        DynamicImage::ImageRgba8(img)
    }
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
