//! The PC Settings label, assembled from the player's own Options artwork.
//! No Nintendo glyph images are included in the executable.

use crate::{
    memory::{MemoryRegion, MemoryState, Permission},
    System,
};

const SIZE: u32 = 0x2000;
const OBJECT: usize = 0x1F00;
const DESCRIPTOR: usize = 0x1F20;
const SHEET: usize = 0x100;
const WIDTH: usize = 128;
const HEIGHT: usize = 64;

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

/// Installs a private CFNU and SDK font object in the system font memory pool.
/// The mapping is saved with guest memory and checked again after state loads.
pub(super) fn install(system: &mut System) -> Option<u32> {
    let mut cursor = 0;
    loop {
        let candidate = system
            .memory
            .mappings()
            .find(|mapping| {
                mapping.base >= cursor
                    && mapping.size == SIZE
                    && mapping.state == MemoryState::Shared
            })
            .copied();
        let Some(mapping) = candidate else { break };
        cursor = mapping.base.checked_add(SIZE)?;
        let mut marker = [0; 8];
        system.memory.read_bytes(mapping.base + 0x1F40, &mut marker);
        if marker == *b"KIULABEL" {
            return Some(mapping.base + OBJECT as u32);
        }
    }
    let other = archive(system, "us/menu/220.zrc")?;
    let controls = archive(system, "us/menu/190.zrc")?;
    let others = texture(&other, "MenOptionOthersBt_Lang0001")?;
    let controls = texture(&controls, "MenOptionSousaBt_Lang00000")?;
    // Uppercase entries retain the large initial; lowercase are small caps.
    // Coordinates are half-open, in the source texture's native pixels.
    let glyphs = [
        (b' ', 0, [0, 0, 0, 0]),
        (b'C', 1, [42, 4, 60, 28]),
        (b'P', 0, [76, 12, 93, 34]),
        (b'S', 0, [76, 54, 90, 76]),
        (b'e', 0, [156, 18, 166, 34]),
        (b'g', 1, [131, 205, 140, 221]),
        (b'i', 0, [131, 18, 140, 34]),
        (b'n', 0, [86, 102, 96, 118]),
        (b's', 0, [171, 60, 182, 76]),
        (b't', 0, [114, 60, 126, 75]),
    ];
    let block = system.memory.phys.allocate(MemoryRegion::Base, SIZE)?;
    let base = system.kernel.linear_base + (block.addr - zakuro_common::memory_map::FCRAM_PADDR);
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
    font[0x3C..0x40].copy_from_slice(&[24, 24, 24, 24]);
    put32(&mut font, 0x40, (WIDTH * HEIGHT / 2) as u32);
    for (at, value) in [
        (0x44, 1),
        (0x46, 11),
        (0x48, 5),
        (0x4A, 2),
        (0x4C, WIDTH as u16),
        (0x4E, HEIGHT as u16),
    ] {
        put16(&mut font, at, value);
    }
    put32(&mut font, 0x50, base + SHEET as u32);
    font[0x1100..0x1104].copy_from_slice(b"CWDH");
    put32(&mut font, 0x1104, 0x40);
    put16(&mut font, 0x110A, glyphs.len() as u16 - 1);
    font[0x1140..0x1144].copy_from_slice(b"CMAP");
    put32(&mut font, 0x1144, 0x16 + glyphs.len() as u32 * 4);
    put16(&mut font, 0x1148, b' ' as u16);
    put16(&mut font, 0x114A, b't' as u16);
    put16(&mut font, 0x114C, 2);
    put16(&mut font, 0x1154, glyphs.len() as u16);
    for (index, (code, source, [x0, y0, x1, y1])) in glyphs.into_iter().enumerate() {
        let (pixels, width) = if source == 0 { others } else { controls };
        let (w, h) = (x1 - x0, y1 - y0);
        if x1 > width || y1 * width / 2 > pixels.len() {
            return None;
        }
        let metric = 0x1110 + index * 3;
        font[metric + 1] = w as u8;
        font[metric + 2] = if code == b' ' { 7 } else { w as u8 + 1 };
        put16(&mut font, 0x1156 + index * 4, code as u16);
        put16(&mut font, 0x1158 + index * 4, index as u16);
        for y in 0..h {
            for x in 0..w {
                let (sx, sy) = (x0 + x, y0 + y);
                let source_pixel = (sy / 8 * (width / 8) + sx / 8) * 64 + morton(sx % 8, sy % 8);
                let alpha = (pixels[source_pixel / 2]
                    >> (if source_pixel.is_multiple_of(2) { 4 } else { 0 }))
                    & 15;
                let (dx, dy) = (index % 5 * 25 + 1 + x, index / 5 * 25 + 1 + 24 - h + y);
                let dest = (dy / 8 * (WIDTH / 8) + dx / 8) * 64 + morton(dx % 8, dy % 8);
                font[SHEET + dest / 2] |= alpha << (if dest.is_multiple_of(2) { 0 } else { 4 });
            }
        }
    }
    put32(&mut font, OBJECT, 0x00D1_5934);
    put32(&mut font, OBJECT + 4, base);
    put32(&mut font, OBJECT + 8, base + 0x1C);
    put32(&mut font, OBJECT + 12, base + DESCRIPTOR as u32);
    put32(&mut font, OBJECT + 16, 6);
    put32(&mut font, OBJECT + 20, 0xFFFF_0000);
    put32(&mut font, DESCRIPTOR + 4, base + OBJECT as u32);
    put32(&mut font, DESCRIPTOR + 8, (block.addr + SHEET as u32) >> 3);
    put16(&mut font, DESCRIPTOR + 12, HEIGHT as u16);
    put16(&mut font, DESCRIPTOR + 14, WIDTH as u16);
    put32(&mut font, DESCRIPTOR + 16, 11);
    font[0x1F40..0x1F48].copy_from_slice(b"KIULABEL");
    system
        .memory
        .map(base, block.addr, SIZE, Permission::RW, MemoryState::Shared);
    system.memory.write_bytes(base, &font);
    Some(base + OBJECT as u32)
}
