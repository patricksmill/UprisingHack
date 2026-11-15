//! The PC page's Row labels, set in the game's own lettering: glyphs cut
//! from Controls' label atlas as the game loaded it, composed into the four
//! On/Off Rows' places and Try It!'s in a copy of that atlas.

use crate::System;

/// Controls' and More Controls' labels, ON, OFF and Try It!: 256 by 256, L4.
const ATLAS: &[u8] = b"MenOptionSousaBt_Lang00000";
const SIDE: usize = 256;
pub(super) const SIZE: usize = SIDE * SIDE / 2;
/// the USA atlas the glyph boxes below were measured on (FNV-1a).
const ATLAS_HASH: u64 = 0xA25A_A2B4_BA7D_1924;
/// each label has a strip this tall, its baseline `BASELINE` rows down.
const STRIP: usize = 32;
const BASELINE: usize = 29;
/// the strips of More Controls' Rows from the top (Guide Arrow, Shot Homing,
/// Aim Assist, Autofire), and of Try It!.
const ROW_STRIPS: [usize; 4] = [128, 192, 160, 96];
const BUTTON_STRIP: usize = 224;
/// a Row's icon and label, left of its ON box; and the button's label.
const LABEL: (usize, usize) = (0, 150);
const LABEL_START: usize = 8;
const BUTTON: (usize, usize) = (0, 128);
/// "More" as More Controls' own label has it: its strip, columns, and where
/// it goes in the button's, whose baseline is two rows higher.
const MORE: (usize, usize, usize) = (64, 42, 92);
const MORE_AT: (usize, usize) = (43, 2);
const WORD_SPACE: usize = 4;

/// a glyph: its strip's top, its columns, and how far its strip's baseline
/// sits above the others'. Uppercase are the large initials, lowercase the
/// small capitals. Adjacent letters touch in the artwork, so the columns
/// are cut where the least ink crosses.
const GLYPHS: [(u8, usize, usize, usize, usize); 18] = [
    (b'H', 192, 76, 91, 0),
    (b'M', 64, 42, 58, 0),
    (b'R', 32, 42, 57, 0),
    (b'S', 192, 28, 43, 0),
    (b'T', 224, 12, 25, 2),
    (b'a', 32, 159, 170, 0),
    (b'd', 128, 59, 69, 0),
    (b'e', 64, 80, 92, 0),
    (b'f', 96, 76, 88, 0),
    (b'g', 192, 130, 140, 0),
    (b'h', 192, 43, 53, 0),
    (b'i', 128, 51, 59, 0),
    (b'l', 32, 101, 110, 0),
    (b'm', 32, 170, 182, 0),
    (b'o', 64, 58, 69, 0),
    (b'r', 64, 69, 80, 0),
    (b's', 160, 98, 110, 0),
    (b't', 160, 130, 142, 0),
];
/// the atlas has no large F: the small one, drawn this much larger about
/// the baseline.
const LARGE: f32 = 23.0 / 16.0;

type Column = [u8; STRIP];

/// where a pixel's nibble is: 8 by 8 tiles in Morton order, the low nibble
/// first.
fn nibble(x: usize, y: usize) -> (usize, u32) {
    let morton = (x & 1) | (y & 1) << 1 | (x & 2) << 1 | (y & 2) << 2 | (x & 4) << 2 | (y & 4) << 3;
    let pixel = (y / 8 * (SIDE / 8) + x / 8) * 64 + morton;
    (pixel / 2, if pixel.is_multiple_of(2) { 0 } else { 4 })
}

fn get(atlas: &[u8], x: usize, y: usize) -> u8 {
    let (at, shift) = nibble(x, y);
    (atlas[at] >> shift) & 15
}

fn set(atlas: &mut [u8], x: usize, y: usize, value: u8) {
    let (at, shift) = nibble(x, y);
    atlas[at] = atlas[at] & !(15 << shift) | (value & 15) << shift;
}

/// the columns of a strip of the atlas, lowered by `lower` rows.
fn columns(atlas: &[u8], top: usize, x0: usize, x1: usize, lower: usize) -> Vec<Column> {
    (x0..x1).map(|x| std::array::from_fn(|y| if y >= lower { get(atlas, x, top + y - lower) } else { 0 })).collect()
}

fn glyph(atlas: &[u8], letter: u8) -> Option<Vec<Column>> {
    if letter == b'F' {
        return Some(enlarge(&glyph(atlas, b'f')?));
    }
    let &(_, top, x0, x1, lower) = GLYPHS.iter().find(|glyph| glyph.0 == letter)?;
    Some(columns(atlas, top, x0, x1, lower))
}

/// a small capital drawn as a large one, about the baseline, its edges
/// firmed up again after the scaling softens them.
fn enlarge(small: &[Column]) -> Vec<Column> {
    let width = (small.len() as f32 * LARGE).round() as usize;
    let sample = |x: usize, y: usize| small.get(x).map_or(0.0, |column| column[y.min(STRIP - 1)] as f32);
    (0..width).map(|x| std::array::from_fn(|y| {
        let source_y = BASELINE as f32 - (BASELINE as f32 - y as f32) / LARGE;
        if source_y < 0.0 { return 0; }
        let source_x = x as f32 / LARGE;
        let (x0, y0) = (source_x as usize, source_y as usize);
        let (fx, fy) = (source_x - x0 as f32, source_y - y0 as f32);
        let value = (sample(x0, y0) * (1.0 - fx) + sample(x0 + 1, y0) * fx) * (1.0 - fy)
            + (sample(x0, y0 + 1) * (1.0 - fx) + sample(x0 + 1, y0 + 1) * fx) * fy;
        ((value - 7.5) * 1.8 + 7.5).round().clamp(0.0, 15.0) as u8
    })).collect()
}

/// a label's columns, narrowed evenly when it is wider than its place, as
/// the game narrows Autofire.
fn label(atlas: &[u8], text: &str, room: usize) -> Option<Vec<Column>> {
    let mut all = Vec::new();
    for letter in text.bytes() {
        if letter == b' ' {
            all.extend(std::iter::repeat_n([0; STRIP], WORD_SPACE));
        } else {
            all.extend(glyph(atlas, letter)?);
        }
    }
    if all.len() <= room { return Some(all); }
    let scale = all.len() as f32 / room as f32;
    Some((0..room).map(|x| {
        let at = x as f32 * scale;
        let (x0, fx) = (at as usize, at.fract());
        let next = all.get(x0 + 1).unwrap_or(&all[x0]);
        std::array::from_fn(|y| (all[x0][y] as f32 * (1.0 - fx) + next[y] as f32 * fx).round() as u8)
    }).collect())
}

/// the atlas with the four Rows' labels and icons replaced by `labels`,
/// from the top Row down, and Try It! by More. None when a label needs a
/// letter the atlas has no glyph for, or the atlas is not the one measured.
pub(super) fn repaint(original: &[u8], labels: [&str; 4]) -> Option<Vec<u8>> {
    if original.len() != SIZE || hash(original) != ATLAS_HASH { return None; }
    let mut atlas = original.to_vec();
    for (text, top) in labels.into_iter().zip(ROW_STRIPS) {
        let columns = label(original, text, LABEL.1 - LABEL_START)?;
        clear(&mut atlas, top, LABEL);
        place(&mut atlas, top, LABEL_START, &columns);
    }
    clear(&mut atlas, BUTTON_STRIP, BUTTON);
    place(&mut atlas, BUTTON_STRIP, MORE_AT.0, &columns_raised(original, MORE, MORE_AT.1));
    Some(atlas)
}

/// a strip's columns raised by `raise` rows.
fn columns_raised(atlas: &[u8], (top, x0, x1): (usize, usize, usize), raise: usize) -> Vec<Column> {
    (x0..x1).map(|x| std::array::from_fn(|y| if y + raise < STRIP { get(atlas, x, top + y + raise) } else { 0 })).collect()
}

fn clear(atlas: &mut [u8], top: usize, (x0, x1): (usize, usize)) {
    for y in top..top + STRIP {
        for x in x0..x1 { set(atlas, x, y, 0); }
    }
}

fn place(atlas: &mut [u8], top: usize, x0: usize, columns: &[Column]) {
    for (x, column) in columns.iter().enumerate() {
        for (y, &value) in column.iter().enumerate() { set(atlas, x0 + x, top + y, value); }
    }
}

fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xCBF2_9CE4_8422_2325, |hash, &byte| (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01B3))
}

/// where the game has the atlas's pixels, by the texture object in the
/// linear heap named for it: 'TXOB' at +4, its name at +0xC (an offset from
/// there until the game makes it a pointer), L4 (10) at +0x34, 256 by 256
/// at +0x3C and +0x40, and the pixels' address at +0x54.
pub(super) fn find(system: &mut System) -> Option<u32> {
    const CHUNK: u32 = 0x10_0000;
    let base = system.kernel.linear_base;
    let mut chunk = vec![0u8; CHUNK as usize + 0x60];
    for start in (base..base + 0x0800_0000).step_by(CHUNK as usize) {
        if !system.memory.is_readable(start, CHUNK + 0x60) { continue; }
        system.memory.read_bytes(start, &mut chunk);
        for at in (4..CHUNK as usize + 4).step_by(4) {
            if &chunk[at..at + 4] != b"TXOB" { continue; }
            let object = at - 4;
            let word = |offset: usize| u32::from_le_bytes(chunk[object + offset..object + offset + 4].try_into().unwrap());
            if word(0x34) != 10 || word(0x3C) != SIDE as u32 || word(0x40) != SIDE as u32 || word(0x44) != SIZE as u32 { continue; }
            let address = start + object as u32;
            let name = word(0xC);
            if !named(system, name) && !named(system, (address + 0xC).wrapping_add(name)) { continue; }
            let pixels = word(0x54);
            if system.memory.is_readable(pixels, SIZE as u32) { return Some(pixels); }
        }
    }
    None
}

fn named(system: &mut System, name: u32) -> bool {
    let mut bytes = [0u8; ATLAS.len() + 1];
    if !system.memory.is_readable(name, bytes.len() as u32) { return false; }
    system.memory.read_bytes(name, &mut bytes);
    bytes[..ATLAS.len()] == *ATLAS && bytes[ATLAS.len()] == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_round_trip_through_the_tile_order() {
        let mut atlas = vec![0u8; SIZE];
        set(&mut atlas, 0, 0, 9);
        set(&mut atlas, 1, 0, 4);
        set(&mut atlas, 255, 255, 15);
        assert_eq!(atlas[0], 0x49, "the first pixel is the low nibble");
        assert_eq!((get(&atlas, 0, 0), get(&atlas, 1, 0), get(&atlas, 255, 255)), (9, 4, 15));
        assert_eq!(atlas[SIZE - 1], 0xF0);
    }

    #[test]
    fn every_label_has_its_glyphs_and_only_the_measured_atlas_is_repainted() {
        let blank = vec![0u8; SIZE];
        for text in super::super::menu_top::PC_PAGE_LABELS {
            let columns = label(&blank, text, LABEL.1 - LABEL_START).expect(text);
            assert!(columns.len() <= LABEL.1 - LABEL_START);
        }
        assert!(label(&blank, "Quiz", 100).is_none());
        assert!(repaint(&blank, super::super::menu_top::PC_PAGE_LABELS).is_none());
    }
}
