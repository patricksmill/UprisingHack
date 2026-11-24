//! Dynamic Input Textures, after Dolphin's: a title's textures that show the
//! console's buttons get the player's own keys and controller buttons drawn
//! over those buttons, following the bindings as they change.
//!
//! A config is JSON in Dolphin's Dynamic Input Textures format
//! (specification 1). Each texture it names, by the texture pack name a
//! dump gives it (`tex1_<w>x<h>_<hash>_<format>_mip0.png`), lists regions
//! of the console's inputs (`emulated_controls`), and the pictures that
//! stand for each host key (`host_controls`, `default_host_controls`). Where
//! Dolphin needs both, Zakuro does without:
//! - with no `image`, the base is the game's own texture, `scale` (4) times
//!   its size, and regions are in the game texture's texels, so a config
//!   carries no picture of the game;
//! - an input whose host key has no picture is drawn: a key as its name
//!   alone, a mouse button, a controller button as the font's symbols are;
//! - a region can be an object, `{"rect": [...], ...}`, giving `fill` (a
//!   colour painted over the region first), `fill_shape` (`rect` or
//!   `ellipse`), `blend` (`replace`, the default, or `over`), `tint` (the
//!   drawn symbol's colour) and `outline` (a colour around it, or null);
//!   the same keys on a texture are its regions' defaults.
//!
//! The console's inputs are A, B, X, Y, L, R, ZL, ZR, Home, DPad, Up, Down,
//! Left, Right, UpDown, LeftRight and CirclePad (`Buttons/A` and other
//! Dolphin-style names work too). Any other name is one of the title's
//! actions, as the frontend gives them to prompts (`NextIdol`, `ZoomIn`):
//! the region draws the input that action is bound to, or stays the game's
//! while it is bound to nothing. Host devices are `Keyboard Mouse`,
//! `Controller`, or "" for either; host keys are named as the bindings
//! name them (`Space`, `Left Click`, `Right Stick`, a controller's `A`).
//! While the console's own buttons are the inputs, or an input's symbol is
//! the console's, the game's picture stays.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use zakuro_gpu::dynamic::{DynamicTextures, Image, Patch, Texture};

use crate::services::prompts::{Host, Input, Label, MouseButton, Prompts};

/// the configs Zakuro carries for a title, read before the mods folder's.
const BUILT_IN: [(u64, &str, &str); 1] = [(0x0004_0000_0003_0100, "Kid Icarus: Uprising", include_str!("../../../kiu/dynamic_input_textures/kiu.json"))];

/// the scale a texture is drawn at when the game's own is its base.
const DEFAULT_SCALE: u32 = 4;

/// every config read, the textures they name.
#[derive(Debug, Default)]
pub struct Config {
    textures: Vec<Output>,
}

/// a texture a config names.
#[derive(Debug)]
struct Output {
    hash: u64,
    width: u32,
    height: u32,
    base: Option<Image>,
    scale: u32,
    preserve_aspect: bool,
    regions: Vec<Region>,
    hosts: Vec<HostControls>,
    /// the folder its pictures are in
    dir: PathBuf,
}

#[derive(Debug, Clone)]
struct Region {
    control: Control,
    /// left, top, right, bottom in the base's pixels
    rect: [u32; 4],
    style: Style,
}

/// what a region's picture stands for.
#[derive(Debug, Clone, PartialEq)]
enum Control {
    /// a console button
    Input(Input),
    /// one of the title's actions, as the frontend names them to prompts
    Action(String),
}

#[derive(Debug, Clone, PartialEq)]
struct Style {
    fill: Option<[u8; 4]>,
    ellipse: bool,
    over: bool,
    tint: [u8; 4],
    outline: Option<[u8; 4]>,
}

impl Default for Style {
    fn default() -> Self {
        Style { fill: None, ellipse: false, over: false, tint: [255, 255, 255, 255], outline: Some([0, 0, 0, 200]) }
    }
}

/// a device's pictures by host key.
#[derive(Debug, Clone)]
struct HostControls {
    device: Device,
    keys: Vec<(String, PathBuf)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Device {
    Any,
    Only(Host),
}

impl Config {
    /// the configs for a title: Zakuro's own for it, then those in dir (its
    /// mods folder's dynamic_input_textures), which replace a texture both name
    pub fn for_title(program_id: u64, dir: Option<&Path>) -> Config {
        let mut config = Config::default();
        for (_, name, text) in BUILT_IN.iter().filter(|(title, ..)| *title == program_id) {
            config.add(text, Path::new(""), &format!("Zakuro's config for {name}"));
        }
        if let Some(dir) = dir.filter(|dir| dir.is_dir()) {
            config.extend(Config::load(dir));
        }
        config
    }

    /// other's textures added, replacing those both name
    pub fn extend(&mut self, other: Config) {
        for texture in other.textures {
            self.textures.retain(|known| known.hash != texture.hash);
            self.textures.push(texture);
        }
    }

    /// the configs in dir, each .json under it, with their pictures beside them
    pub fn load(dir: &Path) -> Config {
        let mut config = Config::default();
        let mut files = Vec::new();
        json_files(dir, 0, &mut files);
        files.sort();
        for path in files {
            match std::fs::read_to_string(&path) {
                Ok(text) => config.add(&text, path.parent().unwrap_or(dir), &path.display().to_string()),
                Err(error) => log::warn!("could not read {}, {error}", path.display()),
            }
        }
        config
    }

    /// adds the config in text, its pictures in dir, a texture named again
    /// replacing the one before; origin names it in the log.
    pub fn add(&mut self, text: &str, dir: &Path, origin: &str) {
        match parse(text, dir) {
            Ok(textures) => {
                log::info!("Dynamic Input Textures in {origin}, {} textures", textures.len());
                for texture in textures {
                    self.textures.retain(|known| known.hash != texture.hash);
                    self.textures.push(texture);
                }
            }
            Err(error) => log::warn!("could not read the Dynamic Input Textures in {origin}, {error}"),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }

    /// the pictures for the inputs as prompts shows them, none where every
    /// input keeps the game's picture
    pub fn build(&self, prompts: &Prompts) -> Option<Arc<DynamicTextures>> {
        let font = crate::services::shared_font::host_font();
        let mut pictures: HashMap<String, Option<Image>> = HashMap::new();
        let mut textures = HashMap::new();
        for output in &self.textures {
            let patches: Vec<Patch> = output
                .regions
                .iter()
                .filter_map(|region| {
                    if prompts.host() == Host::Console {
                        return None;
                    }
                    let label = match &region.control {
                        Control::Input(input) if *prompts.get(*input) == input.console() => return None,
                        Control::Input(input) => prompts.get(*input),
                        // an action bound to nothing on this device keeps the game's picture
                        Control::Action(name) => prompts.action(name)?,
                    };
                    let [left, top, right, bottom] = region.rect;
                    let (width, height) = (right.saturating_sub(left), bottom.saturating_sub(top));
                    if width == 0 || height == 0 {
                        return None;
                    }
                    let picture = match output.picture(prompts.host(), label) {
                        Some(path) => {
                            let read = pictures.entry(path.display().to_string()).or_insert_with(|| {
                                zakuro_gpu::pack::read_image(&path)
                                    .map_err(|error| log::warn!("could not read the Dynamic Input Texture picture {}, {error}", path.display()))
                                    .ok()
                            });
                            fitted(read.as_ref()?, width, height, output.preserve_aspect)
                        }
                        None => drawn(label, width, height, &region.style, font.as_ref()),
                    };
                    let style = &region.style;
                    Some(Patch { x: left, y: top, picture, fill: style.fill.map(|color| (color, style.ellipse)), over: style.over })
                })
                .collect();
            if patches.is_empty() {
                continue;
            }
            let texture = Texture { width: output.width, height: output.height, base: output.base.clone(), scale: output.scale, patches };
            textures.insert(output.hash, texture);
        }
        if textures.is_empty() {
            return None;
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        format!("{prompts:?}").hash(&mut hasher);
        Some(Arc::new(DynamicTextures::new(textures, hasher.finish())))
    }
}

impl Output {
    /// the picture the config gives the host key label stands for, on host
    fn picture(&self, host: Host, label: &Label) -> Option<PathBuf> {
        let name = host_key(label);
        let wanted = |device: Device| device == Device::Only(host);
        // the host's own device first, then the one for any
        [true, false].into_iter().find_map(|own| {
            self.hosts
                .iter()
                .filter(|controls| if own { wanted(controls.device) } else { controls.device == Device::Any })
                .find_map(|controls| controls.keys.iter().find(|(key, _)| same_key(key, &name)).map(|(_, file)| self.dir.join(file)))
        })
    }
}

/// the name the bindings give the host key a label draws.
fn host_key(label: &Label) -> String {
    match label {
        Label::Round(name) | Label::Shoulder(name) | Label::Key(name) => name.clone(),
        Label::Mouse(MouseButton::Left) => "Left Click".to_owned(),
        Label::Mouse(MouseButton::Right) => "Right Click".to_owned(),
        Label::Mouse(MouseButton::Middle) => "Middle Click".to_owned(),
        Label::Stick(name) if name == "L" => "Left Stick".to_owned(),
        Label::Stick(name) if name == "R" => "Right Stick".to_owned(),
        Label::Stick(name) if name.is_empty() => "Circle Pad".to_owned(),
        Label::Stick(name) => name.clone(),
        Label::Cross(None) => "D-Pad".to_owned(),
        Label::Cross(Some(direction)) => format!("D-Pad {direction:?}"),
        Label::Home => "Home".to_owned(),
    }
}

/// key names alike but for case, spaces and Dolphin's backticks
fn same_key(config: &str, name: &str) -> bool {
    let plain = |text: &str| text.chars().filter(|c| !c.is_whitespace() && *c != '`').flat_map(char::to_lowercase).collect::<String>();
    plain(config) == plain(name)
}

/// a picture fitted to width by height, nearest texels, kept in proportion
/// and centred with clear around it when preserve is on, as Dolphin fits them
fn fitted(picture: &Image, width: u32, height: u32, preserve: bool) -> Image {
    let (mut fit_w, mut fit_h) = (width, height);
    if preserve && picture.width > 0 && picture.height > 0 {
        let scale = (width as f32 / picture.width as f32).min(height as f32 / picture.height as f32);
        fit_w = ((picture.width as f32 * scale).round() as u32).clamp(1, width);
        fit_h = ((picture.height as f32 * scale).round() as u32).clamp(1, height);
    }
    let (left, top) = ((width - fit_w) / 2, (height - fit_h) / 2);
    let mut out = Image::new(width, height);
    for y in 0..fit_h {
        for x in 0..fit_w {
            let (sx, sy) = (x * picture.width / fit_w, y * picture.height / fit_h);
            let from = ((sy * picture.width + sx) * 4) as usize;
            let to = (((top + y) * width + left + x) * 4) as usize;
            out.rgba[to..to + 4].copy_from_slice(&picture.rgba[from..from + 4]);
        }
    }
    out
}

/// the label drawn as the font's symbols are, as big as fits in width by
/// height and centred, in the style's tint, outlined.
fn drawn(label: &Label, width: u32, height: u32, style: &Style, font: Option<&ab_glyph::FontRef>) -> Image {
    let side = width.min(height).max(1) as usize;
    // a key is its name alone, no cap, which reads better at a button's size
    let alpha = match label {
        Label::Key(name) => crate::services::prompts::render_text(name, side, font),
        other => crate::services::prompts::render(other, side, font),
    };
    // the outline is the symbol grown by a pixel every 24th of its size
    let grow = (side / 24).max(1) as i32;
    let outline: Vec<u8> = match style.outline {
        Some(_) => (0..side * side)
            .map(|index| {
                let (x, y) = ((index % side) as i32, (index / side) as i32);
                let mut most = 0;
                for dy in -grow..=grow {
                    for dx in -grow..=grow {
                        let (nx, ny) = (x + dx, y + dy);
                        if (0..side as i32).contains(&nx) && (0..side as i32).contains(&ny) && dx * dx + dy * dy <= grow * grow {
                            most = most.max(alpha[ny as usize * side + nx as usize]);
                        }
                    }
                }
                most
            })
            .collect(),
        None => Vec::new(),
    };
    let (left, top) = ((width as usize - side) / 2, (height as usize - side) / 2);
    let mut out = Image::new(width, height);
    for y in 0..side {
        for x in 0..side {
            let index = y * side + x;
            let lit = alpha[index] as f32 / 255.0;
            let edge = outline.get(index).copied().unwrap_or(0) as f32 / 255.0;
            let tint = style.tint;
            let pixel = match style.outline {
                Some(color) if edge > lit => {
                    // the tint over the outline
                    let under = edge * color[3] as f32 / 255.0;
                    let over = lit * tint[3] as f32 / 255.0;
                    let alpha = over + under * (1.0 - over);
                    let channel = |i: usize| {
                        if alpha <= 0.0 { 0 } else { ((tint[i] as f32 * over + color[i] as f32 * under * (1.0 - over)) / alpha).round() as u8 }
                    };
                    [channel(0), channel(1), channel(2), (alpha * 255.0).round() as u8]
                }
                _ => [tint[0], tint[1], tint[2], (lit * tint[3] as f32).round() as u8],
            };
            let at = (((top + y) * width as usize) + left + x) * 4;
            out.rgba[at..at + 4].copy_from_slice(&pixel);
        }
    }
    out
}

fn json_files(dir: &Path, depth: u32, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && depth < 4 {
            json_files(&path, depth + 1, files);
        } else if path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("json")) {
            files.push(path);
        }
    }
}

fn parse(text: &str, dir: &Path) -> Result<Vec<Output>, String> {
    let json: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|error| error.to_string())?;
    match json.get("specification") {
        None => {}
        Some(Value::Number(number)) if number.as_u64() == Some(1) => {}
        Some(other) => return Err(format!("specification {other} is not one Zakuro reads, only 1")),
    }
    let preserve = json["preserve_aspect_ratio"].as_bool().unwrap_or(true);
    let defaults = host_controls(&json["default_host_controls"]);
    let outputs = json["output_textures"].as_object().ok_or("it has no output_textures")?;
    let mut textures = Vec::new();
    for (name, texture) in outputs {
        let Some(hash) = zakuro_gpu::pack::hash_in_name(name) else {
            log::warn!("Dynamic Input Textures: {name} is not a texture name, tex1_<w>x<h>_<hash>_<format>_mip0.png");
            continue;
        };
        let (width, height) = texture_size(name)
            .or_else(|| Some((texture["width"].as_u64()? as u32, texture["height"].as_u64()? as u32)))
            .ok_or_else(|| format!("{name} gives no size, which a name tex1_<w>x<h>_... or width and height do"))?;
        let base = match texture["image"].as_str() {
            Some(file) => Some(zakuro_gpu::pack::read_image(&dir.join(file)).map_err(|error| format!("{name}'s image {file}, {error}"))?),
            None => None,
        };
        let scale = match base {
            Some(_) => 1,
            None => texture["scale"].as_u64().map_or(DEFAULT_SCALE, |scale| scale.clamp(1, 8) as u32),
        };
        let style = style(texture, &Style::default())?;
        let mut regions = Vec::new();
        for (_, controls) in texture["emulated_controls"].as_object().into_iter().flatten() {
            for (control, list) in controls.as_object().into_iter().flatten() {
                // a console button, or else one of the title's actions
                let input = match input(control) {
                    Some(input) => Control::Input(input),
                    None => Control::Action(control.trim_start_matches("Actions/").to_owned()),
                };
                for region in list.as_array().into_iter().flatten() {
                    let (rect, style) = match region {
                        Value::Array(_) => (rect(region)?, style.clone()),
                        Value::Object(_) => (rect(&region["rect"])?, self::style(region, &style)?),
                        other => return Err(format!("{name}'s {control} region {other} is neither [left, top, right, bottom] nor an object")),
                    };
                    regions.push(Region { control: input.clone(), rect: rect.map(|edge| edge * scale), style });
                }
            }
        }
        let mut hosts = host_controls(&texture["host_controls"]);
        hosts.extend(defaults.iter().cloned());
        textures.push(Output { hash, width, height, base, scale, preserve_aspect: preserve, regions, hosts, dir: dir.to_path_buf() });
    }
    Ok(textures)
}

/// the width and height in a name tex1_<w>x<h>_...
fn texture_size(name: &str) -> Option<(u32, u32)> {
    let rest = name.rsplit(['/', '\\']).next()?.strip_prefix("tex1_")?;
    let (size, _) = rest.split_once('_')?;
    let (width, height) = size.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

fn rect(value: &Value) -> Result<[u32; 4], String> {
    let edges: Vec<u32> = value.as_array().ok_or_else(|| format!("{value} is not [left, top, right, bottom]"))?.iter().filter_map(|edge| edge.as_u64().map(|edge| edge as u32)).collect();
    match edges[..] {
        [left, top, right, bottom] if right > left && bottom > top => Ok([left, top, right, bottom]),
        _ => Err(format!("{value} is not [left, top, right, bottom]")),
    }
}

/// a texture's or region's drawing options, those missing as in defaults
fn style(value: &Value, defaults: &Style) -> Result<Style, String> {
    let mut style = defaults.clone();
    if let Some(fill) = value.get("fill") {
        style.fill = optional_color(fill)?;
    }
    if let Some(shape) = value.get("fill_shape").and_then(Value::as_str) {
        style.ellipse = shape.eq_ignore_ascii_case("ellipse");
    }
    if let Some(blend) = value.get("blend").and_then(Value::as_str) {
        style.over = blend.eq_ignore_ascii_case("over");
    }
    if let Some(tint) = value.get("tint") {
        style.tint = optional_color(tint)?.unwrap_or(style.tint);
    }
    if let Some(outline) = value.get("outline") {
        style.outline = optional_color(outline)?;
    }
    Ok(style)
}

/// "#RRGGBB" or "#RRGGBBAA", or null for none
fn optional_color(value: &Value) -> Result<Option<[u8; 4]>, String> {
    let Some(text) = value.as_str() else {
        return if value.is_null() { Ok(None) } else { Err(format!("{value} is not a colour, \"#RRGGBBAA\"")) };
    };
    let hex = text.trim_start_matches('#');
    let channel = |at: usize| u8::from_str_radix(hex.get(at..at + 2).unwrap_or("zz"), 16);
    match hex.len() {
        6 | 8 => {
            let alpha = if hex.len() == 8 { channel(6) } else { Ok(255) };
            match (channel(0), channel(2), channel(4), alpha) {
                (Ok(r), Ok(g), Ok(b), Ok(a)) => Ok(Some([r, g, b, a])),
                _ => Err(format!("{text} is not a colour, \"#RRGGBBAA\"")),
            }
        }
        _ => Err(format!("{text} is not a colour, \"#RRGGBBAA\"")),
    }
}

fn host_controls(value: &Value) -> Vec<HostControls> {
    value
        .as_object()
        .into_iter()
        .flatten()
        .map(|(device, keys)| HostControls {
            device: host_device(device),
            keys: keys.as_object().into_iter().flatten().filter_map(|(key, file)| Some((key.clone(), PathBuf::from(file.as_str()?)))).collect(),
        })
        .collect()
}

/// a device as Dolphin's configs name them, DInput/0/Keyboard Mouse or
/// XInput/0/Gamepad, or as Zakuro does, Keyboard Mouse or Controller
fn host_device(name: &str) -> Device {
    let name = name.to_ascii_lowercase();
    if name.is_empty() {
        Device::Any
    } else if name.contains("keyboard") || name.contains("mouse") {
        Device::Only(Host::KeyboardMouse)
    } else if name.contains("console") || name.contains("3ds") {
        Device::Only(Host::Console)
    } else {
        Device::Only(Host::Controller)
    }
}

/// a console input by its name, case, spaces and a group before a slash aside
fn input(name: &str) -> Option<Input> {
    let plain: String = name.chars().filter(char::is_ascii_alphanumeric).flat_map(|c| c.to_lowercase()).collect();
    let last = name.rsplit('/').next().unwrap_or(name);
    let short: String = last.chars().filter(char::is_ascii_alphanumeric).flat_map(|c| c.to_lowercase()).collect();
    let direction = |text: &str| match text {
        "up" => Some(Input::Up),
        "down" => Some(Input::Down),
        "left" => Some(Input::Left),
        "right" => Some(Input::Right),
        "updown" => Some(Input::UpDown),
        "leftright" => Some(Input::LeftRight),
        _ => None,
    };
    match short.as_str() {
        "a" => Some(Input::A),
        "b" => Some(Input::B),
        "x" => Some(Input::X),
        "y" => Some(Input::Y),
        "l" => Some(Input::L),
        "r" => Some(Input::R),
        "zl" => Some(Input::ZL),
        "zr" => Some(Input::ZR),
        "home" => Some(Input::Home),
        "dpad" | "cross" | "controlpad" => Some(Input::DPad),
        "circlepad" | "circle" | "stick" => Some(Input::CirclePad),
        other => direction(other)
            .or_else(|| direction(plain.trim_start_matches("dpad").trim_start_matches("controlpad")))
            .or_else(|| other.strip_prefix("dpad").and_then(direction)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r##"{
        "specification": 1,
        "output_textures": {
            "tex1_32x16_00000000DEADBEEF_9_mip0.png": {
                "scale": 2,
                "emulated_controls": {
                    "3DS": {
                        "A": [[0, 0, 16, 16]],
                        "Buttons/B": [{"rect": [16, 0, 32, 16], "fill": "#102030FF", "outline": null}]
                    }
                }
            }
        }
    }"##;

    fn config() -> Config {
        let mut config = Config::default();
        config.add(CONFIG, Path::new("."), "test");
        config
    }

    /// regions are in the game texture's texels, scaled with it, and an
    /// object region carries its own options.
    #[test]
    fn a_config_reads_its_regions() {
        let config = config();
        let output = &config.textures[0];
        assert_eq!((output.hash, output.width, output.height, output.scale), (0xDEAD_BEEF, 32, 16, 2));
        assert_eq!(output.regions.len(), 2);
        let b = output.regions.iter().find(|region| region.control == Control::Input(Input::B)).expect("B");
        assert_eq!(b.rect, [32, 0, 64, 32]);
        assert_eq!(b.style.fill, Some([0x10, 0x20, 0x30, 0xFF]));
        assert_eq!(b.style.outline, None);
    }

    /// with the console's own buttons, or a key showing as the console's
    /// symbol, the game's picture stays; a bound key gets drawn.
    #[test]
    fn only_bound_inputs_are_drawn() {
        let config = config();
        assert!(config.build(&Prompts::default()).is_none(), "the console's buttons");
        let mut prompts = Prompts::default();
        prompts.set_host(Host::Controller);
        assert!(config.build(&prompts).is_none(), "a controller showing A as A");
        prompts.set_host(Host::KeyboardMouse);
        prompts.set(Input::A, Label::Key("E".into()));
        let built = config.build(&prompts).expect("A is a key");
        assert!(!built.is_empty());
    }

    /// the config Zakuro carries for Kid Icarus: Uprising reads whole.
    #[test]
    fn the_built_in_config_reads() {
        let config = Config::for_title(0x0004_0000_0003_0100, None);
        assert!(config.textures.len() >= 7, "{}", config.textures.len());
        assert!(config.textures.iter().all(|texture| !texture.regions.is_empty()));
        assert!(Config::for_title(0x0004_0000_0000_0000, None).is_empty());
    }

    /// a region for an action draws the input the action is bound to, and
    /// nothing while it is bound to none.
    #[test]
    fn an_action_draws_its_binding() {
        let mut config = Config::default();
        config.add(r#"{"output_textures": {"tex1_16x16_00000000000000AB_9_mip0.png":
            {"emulated_controls": {"3DS": {"NextIdol": [[0, 0, 16, 16]]}}}}}"#, Path::new("."), "test");
        assert_eq!(config.textures[0].regions[0].control, Control::Action("NextIdol".into()));
        let mut prompts = Prompts::default();
        prompts.set_host(Host::KeyboardMouse);
        assert!(config.build(&prompts).is_none(), "Next Idol bound to nothing");
        prompts.set_action("next_idol", Some(Label::Key("E".into())));
        assert!(config.build(&prompts).is_some());
    }

    /// Dolphin's names for inputs and devices are read too.
    #[test]
    fn names_as_dolphin_gives_them() {
        assert_eq!(input("Buttons/A"), Some(Input::A));
        assert_eq!(input("D-Pad/Up"), Some(Input::Up));
        assert_eq!(input("Circle Pad"), Some(Input::CirclePad));
        assert_eq!(input("Shoulders/L"), Some(Input::L));
        assert_eq!(host_device("DInput/0/Keyboard Mouse"), Device::Only(Host::KeyboardMouse));
        assert_eq!(host_device("XInput/0/Gamepad"), Device::Only(Host::Controller));
        assert_eq!(host_device(""), Device::Any);
        assert!(same_key("`Click 0`", "click0"));
    }

    /// a drawn symbol is lit in its middle and clear in its corners.
    #[test]
    fn a_key_is_drawn_to_fit() {
        let image = drawn(&Label::Round("A".into()), 48, 32, &Style::default(), None);
        assert_eq!((image.width, image.height), (48, 32));
        let alpha = |x: u32, y: u32| image.rgba[((y * 48 + x) * 4 + 3) as usize];
        assert_eq!(alpha(0, 0), 0, "outside the square the symbol fits");
        assert!(alpha(24, 1) > 0, "the ring's top");
    }
}
