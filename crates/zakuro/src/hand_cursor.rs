//! The open and closed hand pointers over the Idol View. Windows has no
//! grab cursor of its own (winit shows the four-way move arrows for it), so
//! these are drawn here: black outline, white fill, at twice their 16-pixel
//! art.

const OPEN: [&str; 16] = [
    "       ##       ",
    "   ## #..###    ",
    "  #..##..#..#   ",
    "  #..##..#..# # ",
    "   #..#..#..##.#",
    "   #..#..#..#..#",
    " ## #.......#..#",
    "#..##..........#",
    "#...#.........# ",
    " #............# ",
    "  #...........# ",
    "  #..........#  ",
    "   #.........#  ",
    "    #.......#   ",
    "     #......#   ",
    "     #......#   ",
];

const CLOSED: [&str; 16] = [
    "                ",
    "                ",
    "                ",
    "    ## ## ##    ",
    "   #..#..#..##  ",
    "   #.........#.#",
    "    #..........#",
    "   ##..........#",
    "  #.#.........# ",
    "  #...........# ",
    "  #...........# ",
    "   #.........#  ",
    "    #........#  ",
    "     #......#   ",
    "     #......#   ",
    "                ",
];

const SCALE: usize = 2;
pub const SIZE: u16 = (16 * SCALE) as u16;
/// the point that acts: the palm's middle.
pub const HOTSPOT: (u16, u16) = (SIZE / 2, SIZE / 2);

/// the hand's pixels, RGBA, SIZE square.
pub fn rgba(closed: bool) -> Vec<u8> {
    let art = if closed { &CLOSED } else { &OPEN };
    let size = SIZE as usize;
    let mut pixels = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let colour = match art[y / SCALE].as_bytes()[x / SCALE] {
                b'#' => [0, 0, 0, 255],
                b'.' => [255, 255, 255, 255],
                _ => [0, 0, 0, 0],
            };
            pixels[(y * size + x) * 4..][..4].copy_from_slice(&colour);
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_art_is_square_and_the_hands_are_drawn() {
        for art in [OPEN, CLOSED] {
            assert!(art.iter().all(|row| row.len() == 16));
        }
        for closed in [false, true] {
            let pixels = rgba(closed);
            assert_eq!(pixels.len(), (SIZE as usize).pow(2) * 4);
            assert!(pixels.chunks(4).any(|pixel| pixel == [255, 255, 255, 255]));
        }
    }
}
