//! The texture pack name of textures dumped from the RomFS.
//!
//! Reads lines of `<raw pixel file> <format number> <width> <height>` on
//! stdin and prints, for each, the name a texture pack (and the Dynamic
//! Input Textures config) gives the picture replacing that texture:
//! `tex1_<w>x<h>_<hash>_<format>_mip0.png`. `kiu/scripts/kiu_textures.py`
//! calls it to add a `pack` line to each texture's notes.

use std::io::BufRead;

use zakuro_gpu::texture::TextureFormat;

fn main() {
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let words: Vec<&str> = line.split_whitespace().collect();
        let [path, format, width, height] = words[..] else {
            println!("-");
            continue;
        };
        let (Ok(format), Ok(width), Ok(height)) = (format.parse(), width.parse(), height.parse()) else {
            println!("-");
            continue;
        };
        let name = std::fs::read(path)
            .ok()
            .and_then(|data| zakuro_gpu::pack::export_name(&data, TextureFormat::from_raw(format), width, height));
        println!("{}", name.as_deref().unwrap_or("-"));
    }
}
