//! Empaquetador ZIP mínimo (método "stored") para entregar el código fuente
//! generado desde el servidor embebido sin dependencias externas.

pub fn create_zip(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();

    for (name, data) in entries {
        let name = name.replace('\\', "/");
        let name_bytes = name.as_bytes();
        let crc = crc32(data);
        let size = data.len() as u32;
        let offset = out.len() as u32;

        // Cabecera local
        push_u32(&mut out, 0x0403_4B50);
        push_u16(&mut out, 20); // versión necesaria
        push_u16(&mut out, 0x0800); // nombres UTF-8
        push_u16(&mut out, 0); // método: stored
        push_u16(&mut out, 0); // hora
        push_u16(&mut out, 0); // fecha
        push_u32(&mut out, crc);
        push_u32(&mut out, size);
        push_u32(&mut out, size);
        push_u16(&mut out, name_bytes.len() as u16);
        push_u16(&mut out, 0); // extra
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(data);

        // Entrada del directorio central
        push_u32(&mut central, 0x0201_4B50);
        push_u16(&mut central, 20); // versión creadora
        push_u16(&mut central, 20); // versión necesaria
        push_u16(&mut central, 0x0800);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u32(&mut central, crc);
        push_u32(&mut central, size);
        push_u32(&mut central, size);
        push_u16(&mut central, name_bytes.len() as u16);
        push_u16(&mut central, 0); // extra
        push_u16(&mut central, 0); // comentario
        push_u16(&mut central, 0); // disco
        push_u16(&mut central, 0); // atributos internos
        push_u32(&mut central, 0); // atributos externos
        push_u32(&mut central, offset);
        central.extend_from_slice(name_bytes);
    }

    let central_offset = out.len() as u32;
    let central_size = central.len() as u32;
    out.extend_from_slice(&central);

    // Fin del directorio central (EOCD)
    push_u32(&mut out, 0x0605_4B50);
    push_u16(&mut out, 0);
    push_u16(&mut out, 0);
    push_u16(&mut out, entries.len() as u16);
    push_u16(&mut out, entries.len() as u16);
    push_u32(&mut out, central_size);
    push_u32(&mut out, central_offset);
    push_u16(&mut out, 0);

    out
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    crc ^ 0xFFFF_FFFF
}
