//! Dynamic Input Textures: game textures with the console's button pictures
//! in them, drawn again with the player's own keys and buttons painted over
//! those parts, the way Dolphin's Dynamic Input Textures do.
//!
//! What goes where is worked out elsewhere (zakuro-core's
//! dynamic_textures reads the configs and draws the inputs): this holds, by
//! the hash of each texture, a base picture and the patches to put on it,
//! and makes the replacing picture the first time the texture is drawn.
//! Like a texture pack's pictures, only the host's GPU draws them.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::pack::{self, Hashing, Material};
use crate::texture::TextureFormat;

/// an RGBA8 picture, rows from the top, the way texels and pack pictures run.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Image {
        Image { width, height, rgba: vec![0; (width * height * 4) as usize] }
    }

    fn at(&self, x: u32, y: u32) -> usize {
        ((y * self.width + x) * 4) as usize
    }
}

/// what goes on a texture: a base, the patches over it, and the size of the
/// game's texture it replaces.
#[derive(Clone, Debug)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    /// the picture the patches go on; none for the game's own texture,
    /// scale times its size.
    pub base: Option<Image>,
    pub scale: u32,
    pub patches: Vec<Patch>,
}

/// a part of the base painted over: at (x, y) of the base, sized as its
/// picture.
#[derive(Clone, Debug)]
pub struct Patch {
    pub x: u32,
    pub y: u32,
    pub picture: Image,
    /// painted first over the patch's box, a colour and whether it fills
    /// the ellipse in the box rather than all of it.
    pub fill: Option<([u8; 4], bool)>,
    /// laid over what is there; otherwise it takes its box's place, see
    /// through where the picture is.
    pub over: bool,
}

/// the textures that get input pictures, by the hash of each game texture.
pub struct DynamicTextures {
    textures: HashMap<u64, Texture>,
    /// mixed into each picture's hash, so the GPU, which keeps a picture by
    /// its hash, takes a set made for other inputs as new pictures.
    generation: u64,
    made: Mutex<HashMap<u64, Option<Arc<Material>>>>,
}

impl DynamicTextures {
    /// textures by the hash in a pack name (tex1_..._<hash>_...) or a bare hash,
    /// generation telling this set from others made before it.
    pub fn new(textures: HashMap<u64, Texture>, generation: u64) -> DynamicTextures {
        DynamicTextures { textures, generation, made: Mutex::new(HashMap::new()) }
    }

    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }

    /// what goes on the texture of this hash, as the pack name has it
    pub fn texture(&self, hash: u64) -> Option<&Texture> {
        self.textures.get(&hash)
    }

    /// the picture replacing a texture of these bytes, its largest size, made
    /// from texels the first time
    pub fn find(&self, data: &[u8], format: TextureFormat, width: u32, height: u32, texels: &[[u8; 4]], rows: &mut Vec<u8>) -> Option<Arc<Material>> {
        // the hashes a texture pack knows a texture by, Citra's first, as dumps and exports name them
        let (hash, texture) = [Hashing::Rows, Hashing::Bytes]
            .into_iter()
            .filter_map(|hashing| pack::hash(hashing, data, format, width, height, rows))
            .find_map(|hash| self.textures.get(&hash).map(|texture| (hash, texture)))?;
        if (texture.width, texture.height) != (width, height) {
            return None;
        }
        let mut made = self.made.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        made.entry(hash)
            .or_insert_with(|| {
                let image = compose(texture, texels, width, height);
                let salted = hash ^ self.generation.rotate_left(17) ^ 0xD1A1_7E57_0000_0000;
                Some(Material::made(salted, pack::mipmapped(image.width, image.height, image.rgba)))
            })
            .clone()
    }
}

/// the base with each patch put on it.
pub fn compose(texture: &Texture, texels: &[[u8; 4]], width: u32, height: u32) -> Image {
    let mut image = match &texture.base {
        Some(base) => base.clone(),
        None => {
            let scale = texture.scale.max(1);
            let mut image = Image::new(width * scale, height * scale);
            for y in 0..image.height {
                for x in 0..image.width {
                    let texel = texels.get(((y / scale) * width + x / scale) as usize).copied().unwrap_or_default();
                    let at = image.at(x, y);
                    image.rgba[at..at + 4].copy_from_slice(&texel);
                }
            }
            image
        }
    };
    for patch in &texture.patches {
        put(&mut image, patch);
    }
    image
}

fn put(image: &mut Image, patch: &Patch) {
    let (w, h) = (patch.picture.width, patch.picture.height);
    for y in 0..h {
        for x in 0..w {
            let (tx, ty) = (patch.x + x, patch.y + y);
            if tx >= image.width || ty >= image.height {
                continue;
            }
            let at = image.at(tx, ty);
            let mut under: [u8; 4] = image.rgba[at..at + 4].try_into().expect("4 bytes");
            if let Some((color, ellipse)) = patch.fill {
                let inside = !ellipse || {
                    let dx = (x as f32 + 0.5) / w as f32 * 2.0 - 1.0;
                    let dy = (y as f32 + 0.5) / h as f32 * 2.0 - 1.0;
                    dx * dx + dy * dy <= 1.0
                };
                if inside {
                    under = color;
                } else if !patch.over {
                    // a box replaced keeps what is outside its ellipse
                    let from = patch.picture.at(x, y);
                    let source: [u8; 4] = patch.picture.rgba[from..from + 4].try_into().expect("4 bytes");
                    image.rgba[at..at + 4].copy_from_slice(&blend(source, under));
                    continue;
                }
            }
            let from = patch.picture.at(x, y);
            let source: [u8; 4] = patch.picture.rgba[from..from + 4].try_into().expect("4 bytes");
            let out = if patch.over || patch.fill.is_some() { blend(source, under) } else { source };
            image.rgba[at..at + 4].copy_from_slice(&out);
        }
    }
}

/// source over destination, straight alpha.
fn blend(source: [u8; 4], under: [u8; 4]) -> [u8; 4] {
    let sa = source[3] as f32 / 255.0;
    let da = under[3] as f32 / 255.0;
    let alpha = sa + da * (1.0 - sa);
    if alpha <= 0.0 {
        return [0, 0, 0, 0];
    }
    let channel = |i: usize| ((source[i] as f32 * sa + under[i] as f32 * da * (1.0 - sa)) / alpha).round() as u8;
    [channel(0), channel(1), channel(2), (alpha * 255.0).round() as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, color: [u8; 4]) -> Image {
        Image { width, height, rgba: color.repeat((width * height) as usize) }
    }

    /// with no base picture the game's texture is the base, scaled, and a
    /// patch takes its box's place, see-through and all.
    #[test]
    fn a_patch_replaces_its_box_of_the_scaled_texture() {
        let texels = vec![[10, 20, 30, 255]; 4 * 4];
        let mut picture = solid(2, 2, [255, 255, 255, 255]);
        picture.rgba[12..16].copy_from_slice(&[0, 0, 0, 0]);
        let texture = Texture { width: 4, height: 4, base: None, scale: 2, patches: vec![Patch { x: 2, y: 2, picture, fill: None, over: false }] };
        let image = compose(&texture, &texels, 4, 4);
        assert_eq!((image.width, image.height), (8, 8));
        assert_eq!(&image.rgba[image.at(0, 0)..][..4], &[10, 20, 30, 255]);
        assert_eq!(&image.rgba[image.at(2, 2)..][..4], &[255, 255, 255, 255]);
        assert_eq!(&image.rgba[image.at(3, 3)..][..4], &[0, 0, 0, 0], "the patch's clear pixel clears the base");
        assert_eq!(&image.rgba[image.at(4, 4)..][..4], &[10, 20, 30, 255], "outside the patch");
    }

    /// a fill paints the box first, and the picture goes over it.
    #[test]
    fn a_fill_goes_under_the_picture() {
        let texels = vec![[200, 200, 200, 255]; 8 * 8];
        let picture = Image { width: 8, height: 8, rgba: [0, 0, 0, 0].repeat(64) };
        let texture = Texture {
            width: 8,
            height: 8,
            base: None,
            scale: 1,
            patches: vec![Patch { x: 0, y: 0, picture, fill: Some(([0, 0, 255, 255], true)), over: false }],
        };
        let image = compose(&texture, &texels, 8, 8);
        assert_eq!(&image.rgba[image.at(4, 4)..][..4], &[0, 0, 255, 255], "the ellipse's middle");
        assert_eq!(&image.rgba[image.at(0, 0)..][..4], &[200, 200, 200, 255], "a corner, outside the ellipse, stays");
    }
}
