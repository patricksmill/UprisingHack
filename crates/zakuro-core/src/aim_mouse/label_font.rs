//! The editable PC Settings PNG, drawn through the native text pass.
//! A missing asset is exported from the player's own Options artwork.

use crate::{
    System,
    memory::{MemoryRegion, MemoryState, Permission},
};

const SIZE: u32 = 0x2000;
const OBJECT: usize = 0x1F00;
const DESCRIPTOR: usize = 0x1F20;
const SHEET: usize = 0x100;
const WIDTH: usize = 256;
const HEIGHT: usize = 32;
const LABEL_WIDTH: usize = 160;
const LABEL_HEIGHT: usize = 24;
const MARKER: &[u8; 8] = b"KIULBL02";

fn word(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn put32(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn put16(data: &mut [u8], at: usize, value: u16) {
    data[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

/// LZ11 backreferences can overlap; copying a slice would be incorrect.
fn decompress(data: &[u8]) -> Option<Vec<u8>> {
    if *data.first()? != 0x11 {
        return None;
    }
    let size = word(data, 0)? >> 8;
    if size == 0 || size > 16 * 1024 * 1024 {
        return None;
    }
    let mut out = Vec::with_capacity(size as usize);
    let mut at = 4;
    while out.len() < size as usize {
        let flags = *data.get(at)?;
        at += 1;
        for bit in (0..8).rev() {
            if out.len() == size as usize {
                break;
            }
            let a = *data.get(at)? as usize;
            at += 1;
            if flags & (1 << bit) == 0 {
                out.push(a as u8);
                continue;
            }
            let b = *data.get(at)? as usize;
            at += 1;
            let (length, distance) = match a >> 4 {
                0 => {
                    let c = *data.get(at)? as usize;
                    at += 1;
                    (((a & 15) << 4 | b >> 4) + 0x11, ((b & 15) << 8 | c) + 1)
                }
                1 => {
                    let c = *data.get(at)? as usize;
                    let d = *data.get(at + 1)? as usize;
                    at += 2;
                    (
                        ((a & 15) << 12 | b << 4 | c >> 4) + 0x111,
                        ((c & 15) << 8 | d) + 1,
                    )
                }
                n => (n + 1, ((a & 15) << 8 | b) + 1),
            };
            if distance > out.len() || length > size as usize - out.len() {
                return None;
            }
            for _ in 0..length {
                out.push(out[out.len() - distance]);
            }
        }
    }
    Some(out)
}

/// Find the named texture through a CGFX dictionary. Relative pointers stay
/// valid inside nested darc files, so no archive copies are needed here.
fn texture<'a>(data: &'a [u8], name: &str) -> Option<(&'a [u8], usize)> {
    for cgfx in data
        .windows(4)
        .enumerate()
        .filter_map(|(at, bytes)| (bytes == b"CGFX").then_some(at))
    {
        let length = word(data, cgfx + 12)? as usize;
        let Some(file) = data.get(cgfx..cgfx.checked_add(length)?) else {
            continue;
        };
        for dict in file
            .windows(4)
            .enumerate()
            .filter_map(|(at, bytes)| (bytes == b"DICT").then_some(at))
        {
            let count = word(file, dict + 8)? as usize;
            if count > file.len() / 16 {
                continue;
            }
            for index in 0..count {
                let entry = dict + 0x1C + index * 16;
                let Some(name_offset) = word(file, entry + 8) else {
                    continue;
                };
                let name_at = (entry + 8).wrapping_add(name_offset as i32 as usize);
                if file.get(name_at..name_at.checked_add(name.len())?) != Some(name.as_bytes())
                    || file.get(name_at + name.len()) != Some(&0)
                {
                    continue;
                }
                let object = (entry + 12).wrapping_add(word(file, entry + 12)? as i32 as usize);
                if file.get(object + 4..object + 8)? != b"TXOB" || word(file, object + 0x34)? != 10
                {
                    return None;
                }
                let width = word(file, object + 0x40)? as usize;
                let height = word(file, object + 0x3C)? as usize;
                let size = word(file, object + 0x44)? as usize;
                let pixels = object + 0x48 + word(file, object + 0x48)? as usize;
                if !width.is_multiple_of(8)
                    || !height.is_multiple_of(8)
                    || size != width * height / 2
                {
                    return None;
                }
                return Some((file.get(pixels..pixels.checked_add(size)?)?, width));
            }
        }
    }
    None
}

fn morton(x: usize, y: usize) -> usize {
    (x & 1) | (y & 1) << 1 | (x & 2) << 1 | (y & 2) << 2 | (x & 4) << 2 | (y & 4) << 3
}

fn archive(system: &System, path: &str) -> Option<Vec<u8>> {
    let title = system.title.as_ref()?;
    let entry = title.romfs.as_ref()?.lookup(path).ok()?;
    decompress(title.read_romfs(&entry, 0, entry.data_size as usize)?)
}

/// The editable file travels with the executable, independently of the cwd.
fn asset_path() -> Option<std::path::PathBuf> {
    Some(
        std::env::current_exe()
            .ok()?
            .parent()?
            .join("assets/pc-settings-label.png"),
    )
}

fn read_alpha(input: impl std::io::BufRead + std::io::Seek) -> Result<Vec<u8>, String> {
    let mut decoder = png::Decoder::new_with_limits(input, png::Limits { bytes: 1024 * 1024 });
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    if (reader.info().width, reader.info().height) != (LABEL_WIDTH as u32, LABEL_HEIGHT as u32) {
        return Err(format!("expected {LABEL_WIDTH} x {LABEL_HEIGHT} pixels"));
    }
    let mut pixels = vec![0; reader.output_buffer_size().ok_or("PNG is too large")?];
    let info = reader.next_frame(&mut pixels).map_err(|e| e.to_string())?;
    let channels = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::GrayscaleAlpha => 2,
        _ => return Err("export a PNG with transparency (RGBA)".into()),
    };
    Ok(pixels[..info.buffer_size()]
        .chunks_exact(channels)
        .map(|p| p[channels - 1])
        .collect())
}

fn export_alpha(path: &std::path::Path, alpha: &[u8]) -> Result<(), String> {
    // Never overwrite an edited file, even when it could not be decoded.
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, LABEL_WIDTH as u32, LABEL_HEIGHT as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let rgba: Vec<_> = alpha.iter().flat_map(|a| [255, 255, 255, *a]).collect();
        encoder
            .write_header()
            .map_err(|e| e.to_string())?
            .write_image_data(&rgba)
            .map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(path.parent().ok_or("no asset directory")?)
        .map_err(|e| e.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut file, &bytes).map_err(|e| e.to_string())
}

fn label_alpha(system: &System) -> Option<Vec<u8>> {
    if let Some(path) = asset_path() {
        match std::fs::File::open(&path) {
            Ok(file) => match read_alpha(std::io::BufReader::new(file)) {
                Ok(alpha) => {
                    log::info!("PC Settings label: {}", path.display());
                    return Some(alpha);
                }
                Err(error) => log::warn!(
                    "PC Settings label {}: {error}; using original artwork",
                    path.display()
                ),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let alpha = original_alpha(system)?;
                match export_alpha(&path, &alpha) {
                    Ok(()) => log::info!("Exported editable PC Settings label: {}", path.display()),
                    Err(error) => log::warn!(
                        "Could not export PC Settings label {}: {error}",
                        path.display()
                    ),
                }
                return Some(alpha);
            }
            Err(error) => log::warn!(
                "Could not read PC Settings label {}: {error}; using original artwork",
                path.display()
            ),
        }
    }
    original_alpha(system)
}

fn original_alpha(system: &System) -> Option<Vec<u8>> {
    let other = archive(system, "us/menu/220.zrc")?;
    let controls = archive(system, "us/menu/190.zrc")?;
    let others = texture(&other, "MenOptionOthersBt_Lang0001")?;
    let controls = texture(&controls, "MenOptionSousaBt_Lang00000")?;
    // Uppercase entries retain the large initial; lowercase are small caps.
    // Coordinates are half-open, in the source texture's native pixels.
    // Adjacent letters touch in the artwork: the columns are cut where the
    // least ink crosses and keep each letter's own side bearings, and the
    // rows run to the baseline so the letters stand on one line.
    let glyphs = [
        (b' ', 0, [0, 0, 0, 0]),
        (b'C', 1, [100, 69, 115, 93]),
        (b'P', 0, [75, 12, 92, 34]),
        (b'S', 1, [28, 197, 43, 221]),
        (b'e', 1, [80, 77, 92, 93]),
        (b'g', 1, [130, 205, 140, 221]),
        (b'i', 1, [51, 141, 59, 157]),
        (b'n', 1, [126, 77, 137, 93]),
        (b's', 1, [98, 173, 110, 189]),
        (b't', 1, [130, 173, 142, 189]),
    ];
    const TEXT: &[u8] = b"PC Settings";
    const SPACE: usize = 7;
    let advance = |code: u8| glyphs.iter().find(|g| g.0 == code).map(|g| if code == b' ' { SPACE } else { g.2[2] - g.2[0] });
    let total = TEXT.iter().try_fold(0, |sum, &code| Some(sum + advance(code)?))?;
    let mut alpha = vec![0; LABEL_WIDTH * LABEL_HEIGHT];
    let mut pen = LABEL_WIDTH.checked_sub(total)? / 2;
    for code in TEXT {
        let (_, source, [x0, y0, x1, y1]) = *glyphs.iter().find(|g| g.0 == *code)?;
        let (pixels, width) = if source == 0 { others } else { controls };
        let (w, h) = (x1 - x0, y1 - y0);
        if x1 > width || y1 * width / 2 > pixels.len() || h > LABEL_HEIGHT {
            return None;
        }
        for y in 0..h {
            for x in 0..w {
                let (sx, sy) = (x0 + x, y0 + y);
                let pixel = (sy / 8 * (width / 8) + sx / 8) * 64 + morton(sx % 8, sy % 8);
                // L4 keeps the first pixel of each byte in its low nibble.
                let value = (pixels[pixel / 2] >> if pixel.is_multiple_of(2) { 0 } else { 4 }) & 15;
                alpha[(LABEL_HEIGHT - h + y) * LABEL_WIDTH + pen + x] = value * 17;
            }
        }
        pen += advance(*code)?;
    }
    Some(alpha)
}

/// Reuse the saved mapping, but refresh its pixels from the host asset after
/// a state load. Console state must not restore an older Photoshop edit.
pub(super) fn install(system: &mut System) -> Option<u32> {
    let alpha = label_alpha(system)?;
    install_alpha(system, &alpha)
}

fn install_alpha(system: &mut System, alpha: &[u8]) -> Option<u32> {
    debug_assert_eq!(alpha.len(), LABEL_WIDTH * LABEL_HEIGHT);
    let candidates: Vec<_> = system
        .memory
        .mappings()
        .filter(|m| m.size == SIZE && m.state == MemoryState::Shared)
        .copied()
        .collect();
    let existing = candidates.into_iter().find(|m| {
        let mut marker = [0; 8];
        system.memory.read_bytes(m.base + 0x1F40, &mut marker);
        marker == *MARKER || marker == *b"KIULABEL"
    });
    let (base, paddr) = if let Some(mapping) = existing {
        (mapping.base, mapping.paddr)
    } else {
        let block = system.memory.phys.allocate(MemoryRegion::Base, SIZE)?;
        let base =
            system.kernel.linear_base + (block.addr - zakuro_common::memory_map::FCRAM_PADDR);
        system
            .memory
            .map(base, block.addr, SIZE, Permission::RW, MemoryState::Shared);
        (base, block.addr)
    };
    let mut font = vec![0; SIZE as usize];
    font[..4].copy_from_slice(b"CFNU");
    put16(&mut font, 4, 0xFEFF);
    put16(&mut font, 6, 0x14);
    put32(&mut font, 8, 0x0300_0000);
    put32(&mut font, 12, 0x1200);
    put32(&mut font, 16, 4);
    font[0x14..0x18].copy_from_slice(b"FINF");
    put32(&mut font, 0x18, 0x20);
    font[0x1C..0x24].copy_from_slice(&[1, 25, 0, 0, 0, 24, 24, 1]);
    put32(&mut font, 0x24, base + 0x3C);
    put32(&mut font, 0x28, base + 0x1108);
    put32(&mut font, 0x2C, base + 0x1148);
    font[0x30..0x33].copy_from_slice(&[24, 24, 24]);
    font[0x34..0x38].copy_from_slice(b"TGLP");
    put32(&mut font, 0x38, 0x1100 - 0x34);
    font[0x3C..0x40].copy_from_slice(&[80, 24, 24, 80]);
    put32(&mut font, 0x40, (WIDTH * HEIGHT / 2) as u32);
    for (at, value) in [
        (0x44, 1),
        (0x46, 11),
        (0x48, 2),
        (0x4A, 1),
        (0x4C, WIDTH as u16),
        (0x4E, HEIGHT as u16),
    ] {
        put16(&mut font, at, value);
    }
    put32(&mut font, 0x50, base + SHEET as u32);
    font[0x1100..0x1104].copy_from_slice(b"CWDH");
    put32(&mut font, 0x1104, 0x40);
    put16(&mut font, 0x110A, 1);
    font[0x1140..0x1144].copy_from_slice(b"CMAP");
    put32(&mut font, 0x1144, 0x1E);
    put16(&mut font, 0x1148, 0xE000);
    put16(&mut font, 0x114A, 0xE001);
    put16(&mut font, 0x114C, 2);
    put16(&mut font, 0x1154, 2);
    font[0x1111] = 80;
    font[0x1112] = 80;
    font[0x1114] = 80;
    font[0x1115] = 80;
    put16(&mut font, 0x1156, 0xE000);
    put16(&mut font, 0x1158, 0);
    put16(&mut font, 0x115A, 0xE001);
    put16(&mut font, 0x115C, 1);
    for y in 0..LABEL_HEIGHT {
        for x in 0..LABEL_WIDTH {
            let (dx, dy) = (1 + x + x / 80, 1 + y);
            let dest = (dy / 8 * (WIDTH / 8) + dx / 8) * 64 + morton(dx % 8, dy % 8);
            let value = ((alpha[y * LABEL_WIDTH + x] as u16 + 8) / 17) as u8;
            font[SHEET + dest / 2] |= value << if dest.is_multiple_of(2) { 0 } else { 4 };
        }
    }
    put32(&mut font, OBJECT, 0x00D1_5934);
    put32(&mut font, OBJECT + 4, base);
    put32(&mut font, OBJECT + 8, base + 0x1C);
    put32(&mut font, OBJECT + 12, base + DESCRIPTOR as u32);
    put32(&mut font, OBJECT + 16, 6);
    put32(&mut font, OBJECT + 20, 0xFFFF_0000);
    put32(&mut font, DESCRIPTOR + 4, base + OBJECT as u32);
    put32(&mut font, DESCRIPTOR + 8, (paddr + SHEET as u32) >> 3);
    put16(&mut font, DESCRIPTOR + 12, HEIGHT as u16);
    put16(&mut font, DESCRIPTOR + 14, WIDTH as u16);
    put32(&mut font, DESCRIPTOR + 16, 11);
    font[0x1F40..0x1F48].copy_from_slice(MARKER);
    system.memory.write_bytes(base, &font);
    Some(base + OBJECT as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(width: u32, height: u32, color: png::ColorType, pixels: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(color);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(pixels)
                .unwrap();
        }
        bytes
    }

    #[test]
    fn photoshop_rgba_uses_transparency_and_rejects_wrong_canvas_or_flattened_png() {
        let rgba: Vec<_> = (0..LABEL_WIDTH * LABEL_HEIGHT)
            .flat_map(|i| [12, 34, 56, i as u8])
            .collect();
        let bytes = encoded(160, 24, png::ColorType::Rgba, &rgba);
        let alpha = read_alpha(std::io::Cursor::new(&bytes)).unwrap();
        assert_eq!(
            alpha,
            (0..LABEL_WIDTH * LABEL_HEIGHT)
                .map(|i| i as u8)
                .collect::<Vec<_>>()
        );
        assert!(read_alpha(std::io::Cursor::new(&bytes[..bytes.len() / 2])).is_err());
        let wrong_size = encoded(1, 1, png::ColorType::Rgba, &[0; 4]);
        assert!(
            read_alpha(std::io::Cursor::new(wrong_size))
                .unwrap_err()
                .contains("160 x 24")
        );
        let flattened = encoded(160, 24, png::ColorType::Rgb, &vec![255; 160 * 24 * 3]);
        assert!(
            read_alpha(std::io::Cursor::new(flattened))
                .unwrap_err()
                .contains("transparency")
        );
    }

    #[test]
    fn export_round_trips_without_overwriting_an_edit() {
        let path = std::env::temp_dir().join(format!(
            "zakuro-label-{}-{}.png",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let alpha: Vec<_> = (0..LABEL_WIDTH * LABEL_HEIGHT).map(|i| i as u8).collect();
        export_alpha(&path, &alpha).unwrap();
        assert_eq!(
            read_alpha(std::io::BufReader::new(std::fs::File::open(&path).unwrap())).unwrap(),
            alpha
        );
        std::fs::write(&path, b"an edit, even an invalid PNG").unwrap();
        assert!(export_alpha(&path, &alpha).is_err());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"an edit, even an invalid PNG"
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn native_sheet_keeps_both_halves_and_refreshes_old_saved_fonts() {
        let mut system = System::new(crate::Config::default());
        let alpha: Vec<_> = (0..LABEL_WIDTH * LABEL_HEIGHT)
            .map(|i| ((i * 7) % 16) as u8 * 17)
            .collect();
        let object = install_alpha(&mut system, &alpha).unwrap();
        let base = object - OBJECT as u32;
        let mut font = vec![0; SIZE as usize];
        system.memory.read_bytes(base, &mut font);
        assert_eq!(&font[0x1110..0x1116], &[0, 80, 80, 0, 80, 80]);
        for y in 0..LABEL_HEIGHT {
            for x in 0..LABEL_WIDTH {
                let (dx, dy) = (1 + x + x / 80, 1 + y);
                let pixel = (dy / 8 * (WIDTH / 8) + dx / 8) * 64 + morton(dx % 8, dy % 8);
                let actual =
                    (font[SHEET + pixel / 2] >> if pixel.is_multiple_of(2) { 0 } else { 4 }) & 15;
                assert_eq!(actual * 17, alpha[y * LABEL_WIDTH + x]);
            }
        }
        system.memory.write_bytes(base + 0x1F40, b"KIULABEL");
        let count = system.memory.mappings().count();
        assert_eq!(
            install_alpha(&mut system, &vec![0; LABEL_WIDTH * LABEL_HEIGHT]),
            Some(object)
        );
        assert_eq!(system.memory.mappings().count(), count);
        let mut pixels = vec![0xFF; WIDTH * HEIGHT / 2];
        system.memory.read_bytes(base + SHEET as u32, &mut pixels);
        assert!(pixels.iter().all(|p| *p == 0));
    }
}
