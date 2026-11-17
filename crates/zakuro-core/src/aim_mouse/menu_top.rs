//! Scripted menus drawn by the game's scene renderer, with the same boxes
//! used to place their controls and to undo that placement for input.

use super::*;
use super::screen_transform::LayoutKind;

pub(super) const SCISSOR: u32 = 0x009164A8;
pub(super) const SCISSOR_DONE: u32 = 0x00916580;
pub(super) const BOTTOM_TEXT: u32 = 0x007C912C;
pub(super) const BOTTOM_TRANSLUCENT: u32 = 0x007C90F8;
pub(super) const START: u32 = 0x0072A050;
pub(super) const FINISHED: u32 = 0x0072A0B4;
/// the `CircleShadow` draw (this in r0: positions at +0, count at +0x14),
/// run inside each scene pass: the Idols' floor shadow.
pub(super) const CIRCLE_SHADOW_DRAW: u32 = 0x0080B100;
pub(super) const FONT_CALL: [u32; 7] = [0x00A36E64, 0x00A36EC8, 0x00A36EE4, 0x00A36F00, 0x00A36F1C, 0x00A36F74, 0x00A36FA4];
pub(super) const FONT_DONE: [u32; 7] = [0x00A36E68, 0x00A36ECC, 0x00A36EE8, 0x00A36F04, 0x00A36F20, 0x00A36F78, 0x00A36FA8];

#[derive(Default)]
pub(super) struct MenuTop {
    drawn: Option<(u64, usize, u32, f32)>,
    idol_dialog: bool,
    idol_revealing: bool,
    /// the Idols List page is shown, not one idol's description.
    idols_list: bool,
    controls_more: bool,
    physics: Option<PhysicsCall>,
    drag_target: Option<(u32, [f32; 2])>,
    parts: Vec<Part>,
    /// dev editor moves, by screen index and part index.
    overrides: Vec<(usize, usize, [f32; 4])>,
    /// hidden editor items, by stable Screen and part index.
    hidden: Vec<(usize, usize)>,
    draw_order: Vec<usize>,
    native: Vec<(u32, Vec<u8>)>,
    clip: Option<(Placement, bool)>,
    scissor_back: Option<(u32, [u8; 16])>,
    pointer: Option<[f32; 2]>,
    hit_frame: Option<u64>,
    retrying_hit: bool,
    candidates: Vec<(usize, (f32, f32))>,
    candidate: usize,
    active: Option<(usize, u32, u32, bool)>,
    passthrough: bool,
    started: u64,
    duration: u64,
    /// the top's text being drawn is the Fiend's Cauldron's hint line,
    /// placed by its Hint rule (see cauldron_hint).
    pub(super) cauldron_font: bool,
    pub(super) font_back: Option<(u32, [u8; 16])>,
    text_parent: Option<Box<Saved>>,
    drawing_text: bool,
    /// Chapter Select's Rows: the chapter number of each part from
    /// `CHAPTER_FIRST_ROW` on, the first one shown, the wheel's leftover,
    /// how many are open and fit, and the chapter last outlined.
    chapter_rows: Vec<u32>,
    chapter_scroll: usize,
    chapter_wheel: f32,
    chapter_span: (usize, usize),
    chapter_outlined: Option<u32>,
    pub(super) grab: Option<usize>,
    pub(super) blocked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IdolGroup { Counters, Back, Launcher, Dialog, Fountain, Eggs }

impl IdolGroup {
    /// whether the group draws this mesh node of this model. The counters'
    /// model `MenColVisionBt0005` also holds the bottom screen's scenery:
    /// its pedestal goes with the fountain and its rail with the eggs; its
    /// curtain (`NN_B0002`, `NN_B0000`) and ground (`NN_B0004`) stay off,
    /// the top's own scene being behind, and with the ground the
    /// fountain's mist (`MN_B0001`), a band that ends where the bottom
    /// screen did.
    fn shows(self, name: &str, node: &str) -> bool {
        match (self, name) {
            (Self::Counters, "MenColVisionBt0005") => matches!(node, "BL_B0000" | "coin" | "NN_B00010" | "NN_B0006"),
            (Self::Counters, "MenColVisionBt0007") => true,
            (Self::Dialog, "MenCmnWindowBt0000") => true,
            (Self::Back, "MenColVisionBt9999") => true,
            (Self::Launcher, "MenColVisionBt0000" | "MenColVisionBt0001" | "MenColVisionBt0002" | "MenColVisionBt0003") => true,
            (Self::Fountain, "MenColVisionBt0006") => node != "MN_B0001",
            (Self::Fountain, "MenColVisionBt0005") => node == "NN_B0007",
            (Self::Eggs, "MenColVisionBt0002") => true,
            (Self::Eggs, "MenColVisionBt0005") => node == "NN_B0005",
            _ => false,
        }
    }

    /// whether any of this model's nodes are the group's.
    fn owns(self, name: &str) -> bool {
        match self {
            Self::Counters => matches!(name, "MenColVisionBt0005" | "MenColVisionBt0007"),
            _ => self.shows(name, ""),
        }
    }

    /// the launcher and the scenery around it keep the game's depth,
    /// shading and translucent pass, and the scenery takes no touches.
    fn scenery(self) -> bool {
        matches!(self, Self::Fountain | Self::Eggs)
    }

    fn uncut(self) -> bool {
        !matches!(self, Self::Counters | Self::Dialog)
    }

    fn name(self) -> &'static str {
        match self {
            Self::Counters => "Counters", Self::Back => "Back", Self::Launcher => "Launcher",
            Self::Dialog => "Dialog", Self::Fountain => "Fountain", Self::Eggs => "Egg queue",
        }
    }
}

#[derive(Clone)]
struct Part {
    place: Placement,
    visible: bool,
    /// the game's own top-screen drawing: the details card or the scene
    /// behind a menu. it is placed by `begin`, not drawn as a Control.
    backdrop: bool,
    top: bool,
    model: Option<&'static str>,
    idol: Option<IdolGroup>,
    chapter: bool,
}

impl Part {
    fn file_book(&self) -> Option<usize> {
        match self.model {
            Some("MenSelfileBt0000") => Some(0),
            Some("MenSelfileBt0001") => Some(1),
            Some("MenSelfileBt0002") => Some(2),
            _ => None,
        }
    }

    fn hit(&self, point: [f32; 2]) -> bool {
        if !inside(point, self.place.to) { return false; }
        if self.idol == Some(IdolGroup::Dialog) {
            let [x, y] = self.place.unmap_point(point);
            return inside([x, y], [15.0, 20.0, 290.0, 200.0]);
        }
        true
    }
}

impl MenuTop {
    fn file_select(&self) -> bool {
        self.active.map(|(index, ..)| index).or_else(|| self.drawn.map(|(_, index, ..)| index))
            .is_some_and(|index| index < ScreenTransform::scripted_len()
                && matches!(ScreenTransform::scripted(index).layout, LayoutKind::FileSelect))
    }

    fn part(&self, sequence: usize) -> Option<&Part> {
        // A draw order lists the Details card only when split layers replay
        // it last (its words skip the native pass then); otherwise the native
        // pass draws it and no replay may.
        self.parts.get(self.part_index(sequence)?).filter(|part| self.enabled(part) && (!part.backdrop || !self.draw_order.is_empty()))
    }

    fn part_index(&self, sequence: usize) -> Option<usize> {
        if self.draw_order.is_empty() && self.active.is_none() { Some(sequence) } else { self.draw_order.get(sequence).copied() }
    }

    fn enabled(&self, part: &Part) -> bool {
        part.visible && self.available(part)
    }

    fn available(&self, part: &Part) -> bool {
        if self.idol_revealing && matches!(part.idol, Some(IdolGroup::Launcher | IdolGroup::Fountain)) {
            return false;
        }
        match part.model {
            Some("MenOptionControlBt0001" | "MenOptionControlBt0002" | "MenOptionControlBt0006") => !self.controls_more,
            Some("MenOptionControlBt0003" | "MenOptionControlBt0004" | "MenOptionControlBt0005" |
                "MenOptionControlBt0007" | "MenOptionControlBt0008") => self.controls_more,
            _ => true,
        }
    }

    /// The chapter a Chapter Select Row shows, by part index.
    fn row_chapter(&self, index: usize) -> Option<u32> {
        let part = self.parts.get(index)?;
        if !part.chapter || part.top { return None; }
        self.chapter_rows.get(index.checked_sub(CHAPTER_FIRST_ROW)?).copied()
    }
}

/// Chapter Select's List as one row: Rows follow `CHAPTER_LAYERS`, each cut
/// around its chapter's thumbnail.
const CHAPTER_FIRST_ROW: usize = CHAPTER_LAYERS.len();
/// A thumbnail with its outline, in bottom-screen pixels, around its bone.
const CHAPTER_ROW_SOURCE: [f32; 2] = [144.0, 90.0];
const CHAPTER_ROW_SCALE: f32 = 0.52;
const CHAPTER_ROW_GAP: f32 = 4.0;
/// The row's bottom edge, just above Back.
const CHAPTER_ROW_BOTTOM: f32 = 214.0;
/// The List model's skeleton, and its bones' world matrices (3x4, 48 bytes).
const MODEL_SKELETON: u32 = 0x22C;
const SKELETON_WORLD: u32 = 0x40;

/// Where the visible Rows go: their row index and box, after the first
/// `scroll`, and how many fit `width`. A short List starts at the left.
fn chapter_slots(width: f32, count: usize, scroll: usize) -> (Vec<(usize, [f32; 4])>, usize) {
    let size = CHAPTER_ROW_SOURCE.map(|value| value * CHAPTER_ROW_SCALE);
    let pitch = size[0] + CHAPTER_ROW_GAP;
    let fit = (((width - CHAPTER_ROW_GAP) / pitch).floor() as usize).max(1);
    let scroll = scroll.min(count.saturating_sub(fit));
    let slots = (scroll..count.min(scroll + fit)).map(|row| {
        (row, [CHAPTER_ROW_GAP + (row - scroll) as f32 * pitch, CHAPTER_ROW_BOTTOM - size[1], size[0], size[1]])
    }).collect();
    (slots, fit)
}

/// The bottom-screen pixel a List bone's world position draws at, measured
/// from the native strip with the bottom lens at its own 4:3: 8 units a
/// chapter is 77.4 pixels across.
fn chapter_center(x: f32, y: f32) -> [f32; 2] {
    [160.7 + 9.68 * x, 123.9 - 10.22 * y]
}

/// Open chapters, by number, with their thumbnails' centers on the bottom
/// screen as the game has scrolled them this frame. Locked chapters' mesh
/// nodes are hidden by the game.
fn chapter_list(system: &mut System, bottom: u32) -> Vec<(u32, [f32; 2])> {
    let scene = read_word(system, bottom + 0x40);
    let layers = read_word(system, scene + SCENE_LAYERS);
    for layer in BOTTOM_LAYERS {
        let list = read_word(system, layers + layer * LAYER_SIZE + 4);
        if list == 0 { continue; }
        let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
        if end < start || end - start > 0x1000 { continue; }
        for at in (start..end).step_by(4) {
            let model = read_word(system, at);
            if model_name(system, model) == "MenStageselectBt0000" { return chapter_bones(system, model); }
        }
    }
    Vec::new()
}

fn chapter_bones(system: &mut System, model: u32) -> Vec<(u32, [f32; 2])> {
    let cmdl = read_word(system, model + 8);
    let dictionary = read_word(system, cmdl + 0xD0);
    let skeleton = read_word(system, model + MODEL_SKELETON);
    if dictionary == 0 || !system.memory.is_readable(skeleton, SKELETON_WORLD + 4) { return Vec::new(); }
    let dictionary = cmdl + 0xD0 + dictionary;
    let nodes = read_word(system, model + MODEL_NODES);
    let count = read_word(system, dictionary + 8).min(read_word(system, model + MODEL_NODES + 4).wrapping_sub(nodes) / 8);
    let mut open = Vec::new();
    for node in 0..count {
        let at = dictionary + 0x24 + node * 0x10;
        let name_at = at.wrapping_add(read_word(system, at));
        let name = c_string(system, name_at);
        let number = name.strip_prefix("ML_B").filter(|digits| digits.len() == 4).and_then(|digits| digits.parse::<u32>().ok());
        if let Some(number) = number.filter(|_| read_word(system, nodes + node * 8 + 4) != 0) { open.push(number); }
    }
    // the skeleton's SOBJ names its bones in the order of their matrices
    let sobj = read_word(system, skeleton + 8);
    let bones = sobj + 0x1C;
    let bones = bones.wrapping_add(read_word(system, bones));
    let world = read_word(system, skeleton + SKELETON_WORLD);
    let bone_count = read_word(system, bones + 8).min(64);
    let names: Vec<String> = (0..bone_count).map(|bone| {
        let at = bones + 0x24 + bone * 0x10;
        let name_at = at.wrapping_add(read_word(system, at));
        c_string(system, name_at)
    }).collect();
    open.into_iter().filter_map(|number| {
        let bone = names.iter().position(|name| *name == format!("ML_B{number:04}"))? as u32;
        let matrix = world + bone * 48;
        if !system.memory.is_readable(matrix, 48) { return None; }
        let [x, y] = [0xC, 0x1C].map(|at| f32::from_bits(read_word(system, matrix + at)));
        Some((number, chapter_center(x, y)))
    }).collect()
}

fn draw_order(system: &mut System, bottom: u32, parts: &[Part], replay_details: bool, layers_below: &'static [super::screen_transform::TopLayer]) -> Vec<usize> {
    let scene = read_word(system, bottom + 0x40);
    let layers = read_word(system, scene + SCENE_LAYERS);
    let mut names = Vec::new();
    for layer in BOTTOM_LAYERS {
        let list = read_word(system, layers + layer * LAYER_SIZE + 4);
        if list == 0 { continue; }
        let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
        if end >= start && end - start <= 0x1000 {
            for at in (start..end).step_by(4) {
                let model = read_word(system, at);
                let name = model_name(system, model);
                names.push(name);
            }
        }
    }
    let mut order: Vec<usize> = (0..parts.len()).filter(|&index| (!parts[index].backdrop || replay_details)
        && system.aim_mouse.as_ref().is_none_or(|mouse| mouse.menu_top.enabled(&parts[index]))).collect();
    order.sort_by_key(|&index| {
        let part = &parts[index];
        if part.top { return (if part.model.is_some() { 0 } else { usize::MAX }, 0); }
        // Idol Toss's scenery is behind its bowl and Controls.
        if let Some(group) = part.idol { return (1, usize::from(!group.scenery())); }
        // A node layer draws where its model does, in its table's order.
        if let Some(at) = part.model.and_then(|name| layers_below.iter().position(|&(layer, ..)| layer == name)) {
            let owner = layers_below[at].1;
            return (1 + names.iter().position(|model| model == owner).unwrap_or(0), at);
        }
        (1 + part.model.and_then(|name| names.iter().position(|model| model == name)).unwrap_or(0), 0)
    });
    order
}

fn fitted(from: [f32; 4], area: [f32; 4]) -> Placement {
    let scale = (area[2] / from[2]).min(area[3] / from[3]);
    let (width, height) = (from[2] * scale, from[3] * scale);
    Placement { from, to: [area[0] + (area[2] - width) * 0.5, area[1] + (area[3] - height) * 0.5, width, height] }
}

fn inside(point: [f32; 2], rect: [f32; 4]) -> bool {
    point.into_iter().enumerate().all(|(axis, value)| value.is_finite() && value >= rect[axis] && value < rect[axis] + rect[axis + 2])
}

fn mapped(point: [f32; 2], from: [f32; 4], to: [f32; 4]) -> (f32, f32) {
    let [x, y] = Placement { from, to }.map_point(point);
    (x, y)
}

// Avoid dropping an exact native pixel to the previous row after a float
// round trip through the frontend's 400-pixel coordinate space.
fn touch_point(point: (f32, f32)) -> (f32, f32) {
    let pixel = |value: f32, limit: f32| {
        let rounded = value.round();
        (if (value - rounded).abs() < 0.0001 { rounded } else { value }).clamp(0.0, limit)
    };
    (pixel(point.0, 319.0), pixel(point.1, 239.0))
}

fn button_hash(model: &str) -> Option<u32> {
    Some(match model {
        "MenSelfileBt0000" => FILE_PARTS[0], "MenSelfileBt0001" => FILE_PARTS[1],
        "MenSelfileBt0002" => FILE_PARTS[2], "MenSelfileBt0003" => FILE_PARTS[3],
        "MenSelfileBt0004" => FILE_PARTS[4],
        "MenSgHomeBt0001" => 0x94CE5E14, "MenSgHomeBt0002" => 0x79BCE02D,
        "MenSgHomeBt0003" => 0x38C8C232, "MenSgHomeBt0004" => 0xED263933,
        "MenSgHomeBt0005" => 0x71966FCC, "MenSgHomeBt0006" => 0x4F73A4C0,
        "MenTopBt0000" => 0xFD90A21D, "MenTopBt0001" => 0x2E49363F,
        "MenTopBt0002" => 0xE2DCC735, "MenTopBt0003" => 0xE250C577,
        "MenTopBt0004" => 0x5407F273, "MenTopBt0005" => 0xD929D345,
        "MenTopBt0006" => 0x9A07E025, "MenTopBt0007" => 0x4A7AE3E3,
        "MenAlterBt0001" => 0x1B80430F, "MenAlterBt0002" => 0xC59BD435,
        "MenAlterBt0003" => 0xE43D88E9, "MenAlterBt0004" => 0x71966FCC,
        "MenOptionTopBt0001" => 0x6C7DCDF1, "MenOptionTopBt0002" => 0x53977FC2,
        "MenOptionTopBt0003" => 0xFB57B10C,
        "MenTrymissionBt0001" => 0xE50FA312, "MenTrymissionBt0002" => 0xADD1FD65,
        "MenTrymissionBt0003" => 0x71966FCC,
        "MenCollectionTopBt0001" => 0xE1D05CA4, "MenCollectionTopBt0002" => 0x0A9FA648,
        "MenCollectionTopBt0005" => 0xABC93BCF, "MenCollectionTopBt0006" => 0x04E786EA,
        "MenCollectionTopBt0007" => 0x84D28812, "MenCollectionTopBt0003" => 0x9EC09AD0,
        "MenCollectionTopBt0009" => 0xBEA7E6CA,
        "MenEquipBt0001" => 0xA754BFE8, "MenEquipBt0002" => 0xCE77C7CF,
        "MenSgHomeBt9999" | "MenAlterBt9999" | "MenTrymissionBt9999" | "CmnBack0000" | "MenEquipBt9999" => 0x9A07E025,
        "MenOptionTopBt9999" => 0x928BEAA4,
        "MenOptionOtherBt0001" => 0x49C14849,
        "MenOptionOtherBt0002" => 0xF28BAA60,
        "MenOptionOtherBt0004" => 0x9E610C53,
        "MenOptionControlBt0001" => 0xDE4F96DF,
        "MenOptionControlBt0002" => 0x56866028,
        "MenOptionControlBt0006" => 0xFB565F88,
        "MenOptionControlBt0003" => 0x4A0F894B,
        "MenOptionControlBt0004" => 0x8D79D12E,
        "MenOptionControlBt0007" => 0x4A0C9579,
        "MenOptionControlBt0008" => 0x4998C4E7,
        "MenOptionControlBt0005" => 0x49B8FADA,
        "MenOptionCtlTpenBt0001" => 0xE84BA0DD,
        "MenOptionCtlTpenBt0002" => 0x21E46FF0,
        "MenOptionOtherCaptionBt0001" => 0x6A1911BF,
        "MenOptionOtherCaptionBt0002" => 0xE76D363A,
        "MenOptionOtherCaptionBt0003" => 0xDD5F933C,
        "MenOptionSoundBt0002" => 0x6AFB5539,
        "MenOptionSoundBt0003" => 0xCD949985,
        "MenOptionSoundBt0004" => 0x002A0EEA,
        "MenOptionSoundBt0005" => 0x1C05D789,
        "MenOptionNameBt0001" => 0x4A41C1D7,
        "MenOptionNameBt9999" => 0x9A07E025,
        "MenSelnameBt0001" => 0x3837AABD,
        "MenSelnameBt0002" => 0x4F14658D,
        "MenSelnameBt9999" => 0x3B8915AB,
        "MenOptionOtherBt9999" | "MenOptionControlBt9999" |
        "MenOptionCtlTpenBt9999" | "MenOptionSoundBt9999" => 0x928BEAA4,
        "MenCmnWindowBt0000" => 0xB1EBC922,
        _ => return None,
    })
}

fn hint_of(index: usize) -> Option<[f32; 4]> {
    match index.checked_sub(ScreenTransform::scripted_len()) {
        Some(kind) => hint_extra(kind),
        None => ScreenTransform::scripted(index).layout.hint(),
    }
}

fn details_of(index: usize) -> bool {
    match index.checked_sub(ScreenTransform::scripted_len()) {
        Some(_) => true,
        None => ScreenTransform::scripted(index).layout.details(),
    }
}

/// Hints for the extra child pages past the registry's Scripted Menus.
fn hint_extra(kind: usize) -> Option<[f32; 4]> {
    match kind {
        0 | 3 | 4 => Some([0.0, 210.0, 400.0, 30.0]),
        5 => Some([0.0, 190.0, 400.0, 50.0]),
        6 => Some([0.0, 170.0, 400.0, 70.0]),
        _ => None,
    }
}

// Relative to the original one-screen menu sizes. Input uses these same boxes.
const CONTROL_SCALE: f32 = 0.6;
const HINT_SCALE: f32 = 0.7 * CONTROL_SCALE;

/// Controls cut from one shared Board rather than separated meshes.
fn board_cut(index: usize) -> bool {
    (4..=11).contains(&index) || armory_board(index)
}

/// Whether a cut Screen's Controls are also drawn cut at their boxes.
/// Other, Controls/More Controls, Reticle/Camera and Subtitles (4..=7) keep
/// their boxes for layout and touch but draw each Control whole: frames, the
/// Outline's bounce and Try It! reach past the boxes. Volume shares labels
/// and slider pieces in one mesh; Profile and the two dialogs draw their
/// words in the bottom text pass, which a whole draw skips; the Armory
/// Boards share one Board. Those stay cut.
fn drawn_cut(index: usize) -> bool {
    board_cut(index) && !(4..=7).contains(&index)
}

/// Whether one Control is drawn cut at its box. The Armory Boards draw their
/// Controls whole like the Options pages, so frames and Outlines reach past
/// the boxes, except the Controls whose words come from the bottom text pass
/// (`BOARD_TEXT`, such as Gear's Weapons card): that text follows the box,
/// not a whole draw's projection.
fn part_cut(index: usize, model: Option<&str>) -> bool {
    drawn_cut(index) && !(armory_board(index) && model.is_some() && !board_text(index, model))
}

/// The Idols Screen's top layer that is the Idol View, where an Orbit starts.
const IDOL_VIEW_LAYER: &str = "Idol";
/// what the pointer does with the Idol View (`idol_view_pointer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdolPointer { Hover, Orbit, Pan }

/// Zoom a wheel notch takes, of the game's 0..1.
const ZOOM_STEP: f32 = 0.1;

/// The node layers declared by the Screen's layout row.
fn top_layers(index: usize) -> &'static [super::screen_transform::TopLayer] {
    if index < ScreenTransform::scripted_len() { ScreenTransform::scripted(index).layout.top_layers() } else { &[] }
}

/// The bottom Board's node layers declared by the Screen's layout row.
fn bottom_layers(index: usize) -> &'static [super::screen_transform::TopLayer] {
    if index < ScreenTransform::scripted_len() { ScreenTransform::scripted(index).layout.bottom_layers() } else { &[] }
}

/// The texture layers declared by the Screen's layout row.
fn texture_layers(index: usize) -> &'static [super::screen_transform::TextureLayer] {
    if index < ScreenTransform::scripted_len() { ScreenTransform::scripted(index).layout.texture_layers() } else { &[] }
}

/// A texture layer's own label, model and native box, if the part is one.
fn texture_layer(index: usize, model: Option<&str>) -> Option<super::screen_transform::TextureLayer> {
    texture_layers(index).iter().copied().find(|&(layer, ..)| Some(layer) == model)
}

/// The Placement that draws a texture layer's model: the node layer owning
/// it, or the Details card when no node layer does.
fn texture_owner(index: usize, parts: &[Part], model: &str) -> Option<Placement> {
    let owner = top_layers(index).iter().find(|&&(_, owns, _)| owns == model).map(|&(layer, ..)| layer);
    parts.iter().find(|part| match owner { Some(layer) => part.model == Some(layer), None => part.backdrop }).map(|part| part.place)
}

/// A texture layer's box as shown, kept inside where its texture can draw:
/// the native box moved with its owner's Placement. Its size never exceeds
/// that box and it slides back inside, so the drawing matches the editor.
fn clamp_texture_box(to: [f32; 4], native: [f32; 4], owner: Placement) -> [f32; 4] {
    let area = owner.to_box(native);
    // max then min, not clamp: a full-size box rounds its bounds a hair apart
    let (width, height) = (to[2].max(1.0).min(area[2].max(1.0)), to[3].max(1.0).min(area[3].max(1.0)));
    [to[0].min(area[0] + area[2] - width).max(area[0]), to[1].min(area[1] + area[3] - height).max(area[1]), width, height]
}

/// The material's texture Scale and Translate (U, V) that draw a texture
/// whose identity box is `native` into `shown`, both in the owner's native
/// pixels. Measured on the Idols emblem: Scale divides the box about its
/// bottom left corner and Translate moves it right and up by its size.
fn texture_transform(native: [f32; 4], shown: [f32; 4]) -> [f32; 4] {
    let [x, y, width, height] = native;
    [width / shown[2].max(0.01), height / shown[3].max(0.01), (shown[0] - x) / width, (y + height - shown[1] - shown[3]) / height]
}

/// Texture coordinator 0 of a CGFX material (`MTOB`): Scale U and V,
/// Rotate, Translate U and V, then flags whose low byte asks the engine to
/// rebuild the matrix from them (it clears it once done).
const MTOB_COORDINATOR: u32 = 0x16C;
const COORDINATOR_SCALE: u32 = 0x10;
const COORDINATOR_TRANSLATE: u32 = 0x1C;
const COORDINATOR_FLAGS: u32 = 0x24;
/// The Idols backdrop's texture-unit-0 commands in its `MTOB`: the border
/// colour, then (after the header for registers 0x81..0x84) the size and
/// the sampler parameters. ClampToBorder with a white border keeps the
/// texture's edge texels from smearing across the mesh when it shrinks
/// or moves; white is "no emblem" in this material.
const MTOB_BORDER: u32 = 0x2F0;
const MTOB_SAMPLER_HEADER: u32 = 0x2F4;
const MTOB_SAMPLER: u32 = 0x2FC;
const SAMPLER_HEADER: u32 = 0x804F_0081;
const WRAP_BORDER: u32 = 0x1100;
const WRAP_MASK: u32 = 0x7700;

/// Write a texture layer's transform into its model's materials. These
/// writes persist: the engine reads them on its own update, and the model
/// is the Screen's, released with it.
fn place_texture(system: &mut System, model: u32, transform: [f32; 4]) {
    if read_word(system, model) != 0x00D14AF4 { return; }
    let (start, end) = (read_word(system, model + 0x164), read_word(system, model + 0x168));
    if start == 0 || end < start || end - start > 128 * 4 || !system.memory.is_readable(start, end - start) { return; }
    for at in (start..end).step_by(4) {
        let material = read_word(system, at);
        if !system.memory.is_readable(material, 0x18) || read_word(system, material) != 0x00D1510C { continue; }
        for offset in [8, 0x14] {
            let mtob = read_word(system, material + offset);
            if mtob == 0 || !system.memory.is_readable(mtob, MTOB_SAMPLER + 4) || read_word(system, mtob + 4) != u32::from_le_bytes(*b"MTOB") { continue; }
            let coordinator = mtob + MTOB_COORDINATOR;
            // Scale U, V; Translate U, V (Rotate, between them, stays 0)
            let fields = [COORDINATOR_SCALE, COORDINATOR_SCALE + 4, COORDINATOR_TRANSLATE, COORDINATOR_TRANSLATE + 4];
            let old = fields.map(|at| f32::from_bits(read_word(system, coordinator + at)));
            if old != transform {
                for (at, value) in fields.into_iter().zip(transform) {
                    system.memory.write_bytes(coordinator + at, &value.to_le_bytes());
                }
                let flags = read_word(system, coordinator + COORDINATOR_FLAGS);
                system.memory.write_bytes(coordinator + COORDINATOR_FLAGS, &(flags | 1).to_le_bytes());
            }
            if read_word(system, mtob + MTOB_SAMPLER_HEADER) == SAMPLER_HEADER {
                system.memory.write_bytes(mtob + MTOB_BORDER, &u32::MAX.to_le_bytes());
                let sampler = read_word(system, mtob + MTOB_SAMPLER) & !WRAP_MASK | WRAP_BORDER;
                system.memory.write_bytes(mtob + MTOB_SAMPLER, &sampler.to_le_bytes());
            }
        }
    }
}

/// Place every texture layer of the Screen's top models from its part.
fn place_textures(system: &mut System, index: usize, top: u32) {
    if texture_layers(index).is_empty() { return; }
    let parts = system.aim_mouse.as_ref().unwrap().menu_top.parts.clone();
    let scene = read_word(system, top + 0x40);
    let layers = read_word(system, scene + SCENE_LAYERS);
    for layer in 0..0x10 {
        let list = read_word(system, layers + layer * LAYER_SIZE + 4);
        if list == 0 { continue; }
        let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
        if end < start || end - start > 0x1000 { continue; }
        for at in (start..end).step_by(4) {
            let model = read_word(system, at);
            let name = model_name(system, model);
            for &(label, owner, native) in texture_layers(index).iter().filter(|&&(_, owner, _)| owner == name) {
                let (Some(part), Some(place)) = (parts.iter().find(|part| part.model == Some(label)), texture_owner(index, &parts, owner)) else { continue };
                // Hidden, the texture moves wholly off its box.
                let transform = if !part.visible { [1.0, 1.0, 4.0, 4.0] } else {
                    let shown = clamp_texture_box(part.place.to, native, place);
                    let [x, y] = place.unmap_point([shown[0], shown[1]]);
                    texture_transform(native, [x, y, shown[2] * place.from[2] / place.to[2], shown[3] * place.from[3] / place.to[3]])
                };
                place_texture(system, model, transform);
            }
        }
    }
}

/// Whether a bottom Board part is one of the Screen's node layers.
fn bottom_layer_part(index: usize, model: Option<&str>) -> bool {
    model.is_some_and(|model| bottom_layers(index).iter().any(|&(layer, ..)| layer == model))
}

/// Whether a Board Control is cut at its box and may show the game's text:
/// the Armory's text Controls and every bottom node layer.
fn board_text(index: usize, model: Option<&str>) -> bool {
    model.is_some_and(|model| BOARD_TEXT.contains(&model)) || bottom_layer_part(index, model)
}

/// Whether a Board part replays the bottom text pass now. Each replay
/// allocates every glyph its box admits, so a node layer carries text only
/// where it owns some: the Idols List's Row names sit inside the full-screen
/// Background's box and overflowed the buffer once per layer, a black frame.
/// whether an Idols Board layer takes the pointer. The game Orbits on any
/// stylus drag of the bottom screen, and most layers' native boxes cover
/// it, so only the Controls do: the Title bar's arrows, the Zoom bar, Back,
/// AR, and on the List page its Rows, frame and Scroll bar. The PC Orbit is
/// the Idol layer's (idol_view.rs), so the rest can be placed anywhere.
fn idols_takes_pointer(model: Option<&str>, list_page: bool) -> bool {
    match model {
        Some("MenColViewlistBt0000" | "MenColViewlistBt0003" | "MenCmnScbar") => list_page,
        Some("Background" | "Controls hint" | "Description" | "Page counter") => false,
        _ => true,
    }
}

fn board_text_now(index: usize, model: Option<&str>, idols_list: bool) -> bool {
    // The List's Rows and its frame's page pill carry the List's words.
    if matches!(model, Some("MenColViewlistBt0000" | "MenColViewlistBt0003")) { return idols_list; }
    if !bottom_layer_part(index, model) { return board_text(index, model); }
    match model {
        Some("Page counter" | "Title bar" | "Description") => !idols_list,
        _ => false,
    }
}

/// Explicit node ownership wins over a model's remaining-node group.
fn top_layer(layers: &'static [super::screen_transform::TopLayer], model: &str, node: &str) -> Option<&'static str> {
    let owns = |owner: &str| owner.strip_suffix('*').map_or(owner == model, |prefix| model.starts_with(prefix));
    layers.iter().find(|(_, owner, nodes)| owns(owner) && nodes.contains(&node))
        .or_else(|| layers.iter().find(|(_, owner, nodes)| owns(owner) && nodes.is_empty()))
        .map(|&(layer, ..)| layer)
}

/// Armory Controls keep the details card beside them.
fn armory_board(index: usize) -> bool {
    index < ScreenTransform::scripted_len() && matches!(ScreenTransform::scripted(index).layout, LayoutKind::Board { .. })
}

/// Armory models drawn with a Control: its cursor and the markers or
/// pieces over it. Replaying the whole Board for each Control exhausts the
/// game's drawing buffer.
fn board_companions(model: &str) -> &'static [&'static str] {
    match model {
        "MenEquipBt0120" => &["MenEquipBt0124", "MenEquipBt0125", "MenEquipBt0130", "MenEquipBt0131"],
        "MenEquipBt0203" => &["MenEquipBt0206", "MenEquipBt0220", "MenEquipBt0221"],
        "MenEquipBt0211" => &["MenEquipBt0215"],
        // the Idols List's Cursor moves with its Rows
        "MenColViewlistBt0003" => &["MenColViewlistBt0004", "MenColViewlistBt0005"],
        _ => &[],
    }
}

/// Armory Controls that show the game's text: names, stats, rows and labels.
const BOARD_TEXT: &[&str] = &[
    "MenEquipBt0001",
    "MenEquipBt0120", "MenEquipBt0121", "MenEquipBt0122", "MenEquipBt0123",
    "MenEquipBt0204", "MenEquipBt0211", "MenEquipBt0212",
    "MenColViewlistBt0000", "MenColViewlistBt0003",
];

/// Chapter Select's layers before its Rows, in editor order: name, whether
/// cut from the top screen (else the bottom), and the source rectangle.
/// The scenery behind them, rainbow band and grey board included, is the
/// backdrop pass. Each part replays its whole screen, so the rectangle is
/// what keeps it from showing the others: the top's three are bands across
/// the screen (a cleared chapter's laurels and the picture's torn edges go
/// past the art's own box), and Back reaches the screen's left edge, where
/// its arrow starts.
const CHAPTER_LAYERS: [(&str, bool, [f32; 4]); 5] = [
    ("Title", true, [0.0, 18.0, 400.0, 48.0]),
    ("Picture", true, [0.0, 66.0, 400.0, 100.0]),
    ("Stats", true, [0.0, 166.0, 400.0, 74.0]),
    ("Back", false, [0.0, 190.0, 64.0, 50.0]),
    ("Hint strip", false, [200.0, 186.0, 90.0, 26.0]),
];

/// Editor overrides are kept for a 400-wide top screen. Wider, a box keeps
/// its share of the room between the edges, or its gap to the edge it
/// overhangs; one spanning the screen stretches. Narrower, it shrinks
/// evenly, keeping its share of the room vertically too.
fn seat_override(to: [f32; 4], width: f32) -> [f32; 4] {
    let scale = (width / 400.0).min(1.0);
    let [x, w] = seat_axis(to[0], to[2], 400.0, width, scale);
    let [y, h] = seat_axis(to[1], to[3], 240.0, 240.0, scale);
    [x, y, w, h]
}

fn seat_axis(at: f32, size: f32, from: f32, to: f32, scale: f32) -> [f32; 2] {
    if size >= from { return [at * to / from, size * to / from]; }
    let shown = size * scale;
    let at = if at < 0.0 { at } else if at > from - size { at + (to - shown) - (from - size) } else { at / (from - size) * (to - shown) };
    [at, shown]
}

/// The inverse of `seat_override`: a box placed at `width`, kept at 400.
fn unseat_override(to: [f32; 4], width: f32) -> [f32; 4] {
    let scale = (width / 400.0).min(1.0);
    let [x, w] = unseat_axis(to[0], to[2], 400.0, width, scale);
    let [y, h] = unseat_axis(to[1], to[3], 240.0, 240.0, scale);
    [x, y, w, h]
}

fn unseat_axis(at: f32, shown: f32, from: f32, to: f32, scale: f32) -> [f32; 2] {
    if shown >= to { return [at * from / to, shown * from / to]; }
    let size = (shown / scale).min(from - 0.01);
    let at = if at < 0.0 { at } else if at > to - shown { at - (to - shown) + (from - size) } else { at / (to - shown) * (from - size) };
    [at, size]
}

fn layout(index: usize, width: f32) -> (Placement, Vec<Part>) {
    let extra = index.checked_sub(ScreenTransform::scripted_len());
    let kind = extra.is_none().then(|| ScreenTransform::scripted(index).layout);
    if matches!(kind, Some(LayoutKind::Chapter)) {
        let native = Placement { from: [0.0, 0.0, 400.0, 240.0], to: [0.0, 0.0, width, 240.0] };
        let scale = 0.48_f32.min((width - 16.0) / 400.0);
        let details = [(width - 400.0 * scale) * 0.5, 4.0];
        let controls = [(width - 320.0 * scale) * 0.5, 122.0];
        // Each layer is cut from the same screen's drawing and starts where
        // the whole screen, shrunk, put it.
        let cut = |origin: [f32; 2], from: [f32; 4]| Placement {
            from, to: [origin[0] + from[0] * scale, origin[1] + from[1] * scale, from[2] * scale, from[3] * scale],
        };
        let part = |top: bool, place: Placement| Part { visible: true, backdrop: false, chapter: true, idol: None, top, model: None, place };
        return (native, CHAPTER_LAYERS.iter().map(|&(_, top, from)| part(top, cut(if top { details } else { controls }, from))).collect());
    }
    let hint = match extra {
        Some(kind) => hint_extra(kind),
        None => kind.and_then(LayoutKind::hint),
    };
    let mut area = [0.0, 0.0, width, 240.0];
    if let Some([_, y, _, height]) = hint {
        area[1] = if y == 0.0 { height * HINT_SCALE } else { 0.0 };
        area[3] -= height * HINT_SCALE;
    }
    let groups = kind.map_or(&[][..], LayoutKind::group);
    let details = kind.is_none_or(LayoutKind::details);
    // The original top scene is the backdrop, never a thumbnail or a
    // cropped panel. Text is replayed separately without any scene meshes.
    let mut native = Placement { from: [0.0, 0.0, 400.0, 240.0], to: [0.0, 0.0, width, 240.0] };
    let mut controls = area;
    if details {
        // Actual weapon/file information remains a complete, proportional
        // card beside its controls. Scenery-only pages need no such card.
        let body = hint.map_or([0.0, 0.0, 400.0, 240.0], |[_, y, _, height]|
            [0.0, if y == 0.0 { height } else { 0.0 }, 400.0, 240.0 - height]);
        if width >= 240.0 * 16.0 / 9.0 - 0.01 {
            native = fitted(body, [area[0], area[1], width / 3.0, area[3]]);
            controls = [width / 3.0 + 4.0, area[1], width * 2.0 / 3.0 - 4.0, area[3]];
        } else {
            native = fitted(body, [area[0], area[1], width, area[3] / 3.0]);
            controls = [area[0], area[1] + area[3] / 3.0 + 4.0, width, area[3] * 2.0 / 3.0 - 4.0];
        }
    }
    let mut parts = Vec::new();
    if matches!(kind, Some(LayoutKind::FileSelect)) {
        parts.reserve(16);
        let mut board = fitted([0.0, 0.0, 320.0, 240.0], controls);
        for axis in 0..2 {
            board.to[axis] += board.to[axis + 2] * (1.0 - CONTROL_SCALE) * 0.5;
            board.to[axis + 2] *= CONTROL_SCALE;
        }
        for (model, from) in groups.iter().map(|&(model, from)| (Some(model), from))
            .chain([(Some("MenDemoBt0000"), [50.0, 195.0, 135.0, 45.0])]) {
            let [x, y] = board.map_point([from[0], from[1]]);
            let to = [x, y, from[2] * board.to[2] / 320.0, from[3] * board.to[3] / 240.0];
            parts.push(Part { visible: true, backdrop: false, chapter: false, idol: None, top: false, model, place: Placement { from, to } });
        }
        parts.push(Part { visible: true, backdrop: false, chapter: false, idol: None, top: true, model: None, place: native });
        parts.push(Part { visible: true, backdrop: false, chapter: false, idol: None, top: false, model: None, place: board });
        for model in ["NN_U0000", "NN_U0004", "NN_U0005", "NN_U0003", "NN_U0006", "NN_U0007_1", "NN_U0007", "NN_U0009"] {
            parts.push(Part { visible: true, backdrop: false, chapter: false, idol: None, top: true, model: Some(model), place: native });
        }
        return (native, parts);
    }
    if kind.is_some_and(LayoutKind::heart) {
        parts.push(Part { visible: true, backdrop: false, chapter: false, idol: None, top: true, model: Some("MenCmnUpHeart0000"), place: fitted([300.0, 138.0, 100.0, 78.0], [0.0, 0.0, 70.0 * CONTROL_SCALE, 55.0 * CONTROL_SCALE]) });
    }
    if !groups.is_empty() && !armory_board(index) { controls = [0.0, 0.0, width, 240.0]; }
    if matches!(kind, Some(LayoutKind::Idol)) {
        let counter_scale = 0.7_f32.min((width - 16.0) / 160.0);
        for (group, from, to) in [
            (IdolGroup::Counters, [0.0, 0.0, IDOL_COUNTERS_WIDTH, 72.0], [0.0, 14.0, IDOL_COUNTERS_WIDTH * counter_scale, 72.0 * counter_scale]),
            (IdolGroup::Back, [0.0, 190.0, 60.0, 50.0], [8.0, 202.0, 36.0, 30.0]),
            (IdolGroup::Launcher, IDOL_LAUNCHER, idol_toss(width).to_box(IDOL_LAUNCHER)),
        ] {
            parts.push(Part { visible: true, backdrop: false, chapter: false, idol: Some(group), top: false, model: None, place: Placement { from, to } });
        }
    } else if groups.is_empty() {
        parts.push(Part { visible: true, backdrop: false, chapter: false, idol: None, top: false, model: None, place: {
            let mut place = fitted([0.0, 0.0, 320.0, 240.0], controls);
            for axis in 0..2 {
                place.to[axis] += place.to[axis + 2] * (1.0 - CONTROL_SCALE) * 0.5;
                place.to[axis + 2] *= CONTROL_SCALE;
            }
            place
        } });
    } else if board_cut(index) {
        // Dynamic labels and slider pieces share native models. Crop each
        // Control from the same Board rather than separating those meshes.
        let mut board = fitted([0.0, 0.0, 320.0, 240.0], controls);
        for axis in 0..2 {
            board.to[axis] += board.to[axis + 2] * (1.0 - CONTROL_SCALE) * 0.5;
            board.to[axis + 2] *= CONTROL_SCALE;
        }
        for &(name, from) in groups {
            let [x, y] = board.map_point([from[0], from[1]]);
            let to = [x, y, from[2] * board.to[2] / 320.0, from[3] * board.to[3] / 240.0];
            parts.push(Part { visible: true, backdrop: false, chapter: false, idol: None, top: false, model: Some(name), place: Placement { from, to } });
        }
        if index == 11 { parts[0].model = None; }
    } else if kind.is_some_and(LayoutKind::heart) {
        let original_height = 240.0 - hint.map_or(0.0, |hint| hint[3] * 0.7);
        let scale = (original_height / 240.0).min(controls[2] / 320.0).min(1.0) * CONTROL_SCALE;
        let hint_scale = HINT_SCALE.min(width / 400.0);
        let hint_height = hint.map_or(0.0, |h| h[3] * hint_scale);
        let baseline = 240.0 - hint_height - 2.0;
        let mo_size = [242.0 * scale, 164.0 * scale];
        let mo_x = (width - mo_size[0]) * 0.5;
        for &(name, from) in groups {
            let size = [from[2] * scale, from[3] * scale];
            let to = match name {
                "MenSgHomeBt0002" => [mo_x, baseline - size[1], size[0], size[1]],
                "MenSgHomeBt0001" => [mo_x - size[0] + 4.0 * scale, baseline - size[1], size[0], size[1]],
                "MenSgHomeBt0003" => [mo_x + mo_size[0] - 14.0 * scale, baseline - size[1], size[0], size[1]],
                "MenSgHomeBt0004" => [width - size[0], 0.0, size[0], size[1]],
                "MenSgHomeBt0005" => [width - size[0], 240.0 - size[1], size[0], size[1]],
                "MenSgHomeBt0006" => [48.0 * scale + 4.0, 240.0 - size[1], size[0], size[1]],
                "MenSgHomeBt9999" => [0.0, 240.0 - size[1], size[0], size[1]],
                _ => [0.0, 0.0, size[0], size[1]],
            };
            parts.push(Part { visible: true, backdrop: false, chapter: false, idol: None, top: false, model: Some(name), place: Placement { from, to } });
        }
    } else {
        let original_height = 240.0 - hint.map_or(0.0, |hint| hint[3] * 0.7);
        let scale = (original_height / 240.0).min(controls[2] / 320.0).min(1.0) * CONTROL_SCALE;
        for &(name, from) in groups {
            // Preserve each button's position between its original edges.
            // A box touching an edge stays flush with that edge as it shrinks.
            let anchor = [from[0] / (320.0 - from[2]), from[1] / (240.0 - from[3])];
            let size = [from[2] * scale, from[3] * scale];
            let bottom = if name == "MenSgHomeBt0006" { hint.map_or(0.0, |hint| hint[3] * HINT_SCALE + 4.0) } else { 0.0 };
            let to = [controls[0] + (controls[2] - size[0]) * anchor[0],
                controls[1] + (controls[3] - bottom - size[1]) * anchor[1], size[0], size[1]];
            parts.push(Part { visible: true, backdrop: false, chapter: false, idol: None, top: false, model: Some(name), place: Placement { from, to } });
        }
    }
    if let Some(mut from) = hint {
        let scale = if matches!(kind, Some(LayoutKind::Idol)) {
            // Keep both complete hint lines readable, with room for Back
            // in narrow windows and for the dialog above the text.
            from = [0.0, 175.0, 400.0, 45.0];
            0.6_f32.min((width - 88.0) / from[2])
        } else { HINT_SCALE.min(width / from[2]) };
        let height = from[3] * scale;
        parts.push(Part { visible: true, backdrop: false, chapter: false, idol: None, top: true, model: None, place: Placement {
            from, to: [(width - from[2] * scale) * 0.5, if from[1] == 0.0 { 0.0 } else { 240.0 - height }, from[2] * scale, height],
        } });
    }
    // Last, so adding it kept every other layer's saved index.
    parts.push(Part { visible: true, backdrop: true, chapter: false, idol: None, top: true, model: None, place: native });
    if matches!(kind, Some(LayoutKind::Idol)) {
        // After the backdrop, so the earlier layers' saved indices are kept.
        for (group, place) in [
            (IdolGroup::Fountain, Placement { from: IDOL_FOUNTAIN, to: idol_toss(width).to_box(IDOL_FOUNTAIN) }),
            (IdolGroup::Eggs, idol_eggs(width)),
        ] {
            parts.push(Part { visible: true, backdrop: false, chapter: false, idol: Some(group), top: false, model: None, place });
        }
    }
    // After the backdrop, so Gear's earlier saved indices are kept; back to front.
    for &(layer, ..) in kind.map_or(&[][..], LayoutKind::top_layers) {
        parts.push(Part { visible: true, backdrop: false, chapter: false, idol: None, top: true, model: Some(layer), place: native });
    }
    // After the node layers, so their saved indices are kept.
    for &(layer, _, from) in kind.map_or(&[][..], LayoutKind::texture_layers) {
        parts.push(Part { visible: true, backdrop: false, chapter: false, idol: None, top: true, model: Some(layer), place: Placement { from, to: native.to_box(from) } });
    }
    (native, parts)
}

/// Idol Toss's bowl, the fountain under it and the egg queue beside it, in
/// the bottom screen's pixels.
const IDOL_LAUNCHER: [f32; 4] = [40.0, 40.0, 240.0, 110.0];
const IDOL_FOUNTAIN: [f32; 4] = [60.0, 60.0, 200.0, 180.0];
const IDOL_EGGS: [f32; 4] = [262.0, 0.0, 58.0, 240.0];

fn idol_egg_group(system: &mut System, model: u32) -> IdolGroup {
    let skeleton = read_word(system, model + MODEL_SKELETON);
    let world = read_word(system, skeleton + SKELETON_WORLD);
    // Eggs move by their root bone; the native camera shows 32 world units across 320 pixels.
    let x = 160.0 + 10.0 * f32::from_bits(read_word(system, world + 0xC));
    if x < IDOL_EGGS[0] { IdolGroup::Launcher } else { IdolGroup::Eggs }
}

/// source width of Counters normally.
const IDOL_COUNTERS_WIDTH: f32 = 160.0;

/// source width of Counters while an idol's name bar shows.
const IDOL_NAME_WIDTH: f32 = 320.0;

fn idol_revealing(system: &mut System, object: u32) -> bool {
    // Native launch/reveal phases; phase 5 returns control to egg tossing.
    object != 0 && matches!(read_word(system, object + 0x430) & 0xFF, 2..=4)
}

/// where the egg queue goes on a top screen this wide: its rail runs the
/// bottom screen's whole height and ends there, so it keeps its size down
/// the right edge.
fn idol_eggs(width: f32) -> Placement {
    Placement { from: IDOL_EGGS, to: [width - IDOL_EGGS[2], 0.0, IDOL_EGGS[2], IDOL_EGGS[3]] }
}

/// where the bottom screen left of the egg queue goes: one transform for
/// the bowl and the fountain, so they stand together as the game drew
/// them, as far from the queue as there, the bowl's top at 62.
fn idol_toss(width: f32) -> Placement {
    let from = [IDOL_LAUNCHER[0], 0.0, IDOL_EGGS[0] - IDOL_LAUNCHER[0], 240.0];
    let scale = 0.68_f32.min(width * 0.6 / 280.0);
    Placement { from, to: [idol_eggs(width).to[0] - from[2] * scale, 62.0 - IDOL_LAUNCHER[1] * scale, from[2] * scale, from[3] * scale] }
}

fn layout_with_dialog(index: usize, width: f32, idol_dialog: bool) -> (Placement, Vec<Part>) {
    let (native, mut parts) = layout(if idol_dialog { 22 } else { index }, width);
    if idol_dialog {
        let place = fitted([0.0, 0.0, 320.0, 240.0], [8.0, 48.0, width - 16.0, 180.0]);
        // before the hint, which precedes the backdrop
        let hint = parts.iter().position(|part| part.top && part.model.is_none() && !part.backdrop).unwrap();
        parts.insert(hint, Part { visible: true, backdrop: false, chapter: false, idol: Some(IdolGroup::Dialog), top: false, model: None, place });
    }
    (native, parts)
}

fn scene_has_idol(system: &mut System, scene: u32) -> bool {
    let layers = read_word(system, scene + SCENE_LAYERS);
    for layer in BOTTOM_LAYERS {
        let list = read_word(system, layers + layer * LAYER_SIZE + 4);
        if list == 0 { continue; }
        let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
        if end < start || end - start > 0x1000 { continue; }
        if (start..end).step_by(4).any(|at| {
            let model = read_word(system, at);
            model_name(system, model) == "MenColVisionBt0005"
        }) { return true; }
    }
    false
}

/// Screens whose graph navigation is redirected by `navigate`.
const NAVIGABLE_SCREENS: [usize; 17] = [0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 20, 21, 24];

/// A Control the editor can link: a graph node, or a weapon category
/// tile (and Back), which the game steps by index rather than by name.
fn navigation_control(screen: usize, part: &Part) -> bool {
    !part.top && (part.model.and_then(button_hash).is_some() || (screen == 20 && part.model.is_some()))
}

impl AimMouse {
    /// The native choice box is horizontal; it has no Up/Down actions.
    pub fn editor_navigation_directions(&self) -> [bool; 4] {
        if self.menu_screen.is_some_and(|(screen, ..)| screen == 11) {
            [false, false, true, true]
        } else { [true; 4] }
    }

    pub fn editor_navigation_controls(&self) -> Vec<(usize, [Option<usize>; 4])> {
        if !self.editing_scripted() { return Vec::new(); }
        let Some((screen, ..)) = self.menu_screen else { return Vec::new() };
        if !NAVIGABLE_SCREENS.contains(&screen) { return Vec::new(); }
        self.menu_top.parts.iter().enumerate().filter(|(_, part)|
            self.menu_top.enabled(part) && navigation_control(screen, part)).map(|(index, _)| {
            let mut targets = [None; 4];
            for (slot, direction) in [NavigationDirection::Up, NavigationDirection::Down,
                NavigationDirection::Left, NavigationDirection::Right].into_iter().enumerate() {
                targets[slot] = self.navigation_links.iter().find(|link|
                    link.screen == screen && link.from == index && link.direction == direction).map(|link| link.to);
            }
            (index, targets)
        }).collect()
    }

    pub fn set_editor_navigation_link(&mut self, from: usize, direction: NavigationDirection, to: Option<usize>) {
        if !self.editing_scripted() { return; }
        let Some((screen, ..)) = self.menu_screen else { return };
        if !NAVIGABLE_SCREENS.contains(&screen) { return; }
        if screen == 11 && matches!(direction, NavigationDirection::Up | NavigationDirection::Down) { return; }
        let valid = |index| self.menu_top.parts.get(index).is_some_and(|part: &Part|
            self.menu_top.enabled(part) && navigation_control(screen, part));
        if !valid(from) || to.is_some_and(|index| !valid(index)) { return; }
        self.navigation_links.retain(|link| !(link.screen == screen && link.from == from && link.direction == direction));
        if let Some(to) = to { self.navigation_links.push(NavigationLink { screen, from, direction, to }); }
    }
}

/// The weapon category step, at its loop head: r5 the screen, r7 the
/// direction's table, r6 the candidate tried, r4 the candidate taken at
/// 0x001C670C, which also handles Back's return to its previous tile.
const CATEGORY_STEP: u32 = 0x001C66CC;
const CATEGORY_TAKE: u32 = 0x001C670C;
const CATEGORY_NONE: u32 = 0x001C66E4;
const CATEGORY: u32 = 0xDC;
const CATEGORY_BEFORE: u32 = 0xE0;
const CATEGORY_BACK: usize = 10;

fn navigate_categories(system: &mut System) {
    if system.cpu.regs[6] != 0 { return; }
    let (object, table) = (system.cpu.regs[5], system.cpu.regs[7]);
    let direction = match table {
        0x00CA3180 => NavigationDirection::Left,
        0x00CA2AA0 => NavigationDirection::Right,
        0x00CA2730 => NavigationDirection::Up,
        0x00CA2E10 => NavigationDirection::Down,
        _ => return,
    };
    let from = read_word(system, object + CATEGORY) as usize;
    let Some(mouse) = system.aim_mouse.as_ref().filter(|mouse| mouse.menus_on_top
        && mouse.menu_screen.is_some_and(|(screen, owner, ..)| screen == 20 && owner == object)) else { return };
    if !mouse.menu_top.parts.get(from).is_some_and(|part| mouse.menu_top.enabled(part)) { return; }
    let Some(to) = mouse.navigation_links.iter().find(|link| link.screen == 20 && link.from == from && link.direction == direction)
        .map(|link| link.to).filter(|&to| to <= CATEGORY_BACK && mouse.menu_top.parts.get(to).is_some_and(|part| mouse.menu_top.enabled(part))) else { return };
    if to == from {
        system.cpu.regs[15] = CATEGORY_NONE;
        return;
    }
    if !category_unlocked(system, to as u32) { return; }
    if from == CATEGORY_BACK {
        // Back always returns to the tile saved here.
        system.memory.write_bytes(object + CATEGORY_BEFORE, &(to as u32).to_le_bytes());
    }
    system.cpu.regs[4] = to as u32;
    system.cpu.regs[15] = CATEGORY_TAKE;
}

/// Redirect the native graph lookup, not pad input. The native caller keeps
/// its repeat, availability checks, outline updates, sound and activation.
pub(super) fn navigate(system: &mut System, pc: u32) {
    if pc == 0x00241368 {
        let object = system.cpu.regs[4];
        let Some(_) = system.aim_mouse.as_ref().filter(|mouse| mouse.menus_on_top
            && !mouse.menu_top.idol_dialog && mouse.menu_screen.is_some_and(|(screen, owner, ..)| screen == 11 && owner == object)) else { return };
        let direction = match system.cpu.regs[14] {
            0x002412A4 => NavigationDirection::Left,
            0x0024130C => NavigationDirection::Right,
            _ => return,
        };
        let from = read_word(system, object + DIALOG_PICKED) as usize + 1;
        let mouse = system.aim_mouse.as_ref().unwrap();
        if !mouse.menu_top.parts.get(from).is_some_and(|part| mouse.menu_top.enabled(part)) { return; }
        if let Some(link) = mouse.navigation_links.iter().find(|link|
            link.screen == 11 && link.from == from && link.direction == direction && (1..=2).contains(&link.to)
                && mouse.menu_top.parts.get(link.to).is_some_and(|part| mouse.menu_top.enabled(part))) {
            system.cpu.regs[0] = link.to as u32 - 1;
        }
        return;
    }
    if matches!(pc, 0x002D590C | 0x002D5B40 | 0x002D5D74 | 0x002D699C) {
        let direction = match pc {
            0x002D590C => NavigationDirection::Up,
            0x002D5B40 => NavigationDirection::Down,
            0x002D5D74 => NavigationDirection::Left,
            _ => NavigationDirection::Right,
        };
        let object = system.cpu.regs[0];
        let hash = read_word(system, object + BUTTONS_OUTLINED);
        let Some(mouse) = system.aim_mouse.as_ref().filter(|mouse| mouse.menus_on_top
            && mouse.menu_screen.is_some_and(|(screen, owner, ..)| screen == 8 && owner == object)) else { return };
        let Some(from) = mouse.menu_top.parts.iter().position(|part|
            mouse.menu_top.enabled(part) && part.model.and_then(button_hash) == Some(hash)) else { return };
        let Some(link) = mouse.navigation_links.iter().find(|link|
            link.screen == 8 && link.from == from && link.direction == direction) else { return };
        let Some(target) = mouse.menu_top.parts.get(link.to).filter(|part| mouse.menu_top.enabled(part))
            .and_then(|part| part.model).and_then(button_hash) else { return };
        // The native repeat controller has already selected this direction.
        // A link replaces navigation, not the slider's value adjustment.
        if read_word(system, system.cpu.regs[1]) & 5 == 0 {
            system.cpu.regs[15] = system.cpu.regs[14];
            return;
        }
        let owner = read_word(system, object + BUTTONS_SOUND_OWNER);
        let descriptor = kept_of_kind(system, owner, 0x00D70ED8);
        if descriptor == 0 { return; }
        for offset in (0x18..0x400).step_by(4) {
            let name = descriptor + offset;
            if read_word(system, name) != target || read_word(system, name + 8) != 0x20 { continue; }
            let text = read_word(system, name + 4);
            if !system.memory.is_readable(text, 1) { continue; }
            let flags = read_word(system, object + QUEUED);
            system.memory.write_bytes(object + QUEUED, &[(flags as u8 & !2) | 1]);
            system.memory.write_bytes(object + VOLUME_HELD, &[0; 8]);
            system.cpu.regs[0] = object + QUEUED_PART;
            system.cpu.regs[1] = name;
            system.cpu.regs[15] = QUEUE_PART;
            return;
        }
        return;
    }
    if pc == CATEGORY_STEP {
        navigate_categories(system);
        return;
    }
    let (direction, slot) = match pc {
        0x003506C4 => {
            let Some(direction) = NavigationDirection::from_native_code(system.cpu.regs[1]) else { return };
            (direction, None)
        }
        // These key handlers inline the same graph lookup.
        0x002BC5A4 | 0x00321720 | 0x0026E0A0 | 0x00284898 | 0x00219A64 | 0x0023C1AC | 0x002E135C |
        0x002474B0 | 0x0019560C | 0x002535C0 | 0x002B75E4 => (NavigationDirection::Left, Some(0x64)),
        0x002BC618 | 0x0032178C | 0x0026E114 | 0x00284948 | 0x00219AD8 | 0x0023C238 | 0x002E1854 |
        0x0024751C | 0x0019568C | 0x00253610 | 0x002B7638 => (NavigationDirection::Right, Some(0x38)),
        0x002BC688 | 0x003217F8 | 0x0026E18C | 0x002849F8 | 0x00219B50 | 0x0023C2C8 | 0x00293848 |
        0x00247588 | 0x0019570C | 0x00253660 | 0x002B7688 => (NavigationDirection::Up, Some(0x90)),
        0x002BC6CC | 0x00321864 | 0x0026E204 | 0x00284AA8 | 0x00219BC8 | 0x0023C384 | 0x002938CC |
        0x002475F4 | 0x00195784 | 0x002536B0 | 0x002B76E0 => (NavigationDirection::Down, Some(0xBC)),
        _ => return,
    };
    let record = system.cpu.regs[0];
    let hash = read_word(system, record + 0xc);
    let Some(mouse) = system.aim_mouse.as_ref().filter(|mouse| mouse.menus_on_top) else { return };
    let Some((screen, ..)) = mouse.menu_screen else { return };
    let Some(from) = mouse.menu_top.parts.iter().position(|part|
        mouse.menu_top.enabled(part) && !part.top && part.model.and_then(button_hash) == Some(hash)) else { return };
    let Some(link) = mouse.navigation_links.iter().find(|link|
        link.screen == screen && link.from == from && link.direction == direction) else { return };
    let Some(target) = mouse.menu_top.parts.get(link.to).filter(|part| mouse.menu_top.enabled(part))
        .and_then(|part| part.model).and_then(button_hash) else { return };
    // The graph is a bounded circular list; each node owns its current name
    // at +0xc and its next/previous pointers at +4/+8.
    let mut at = record;
    for _ in 0..128 {
        if at == 0 || !system.memory.is_readable(at, 0x38) { return; }
        if read_word(system, at + 0xc) == target {
            if matches!(pc, 0x002BC5A4 | 0x002BC618 | 0x002BC688 | 0x002BC6CC) {
                // A graph-slot substitution is insufficient here: Left/Right
                // also steps the book index. The native outline helper updates
                // the destination name, book index and animation together.
                let resume = if matches!(direction, NavigationDirection::Left | NavigationDirection::Right) {
                    0x002BC66C
                } else { 0x002BC6E8 };
                if target == hash {
                    system.cpu.regs[15] = resume;
                } else {
                    system.cpu.regs[0] = system.cpu.regs[5];
                    system.cpu.regs[1] = at + 0xc;
                    system.cpu.regs[14] = resume;
                    system.cpu.regs[15] = OUTLINE_FILE_PART;
                }
                return;
            }
            if let Some(slot) = slot {
                // The caller loads the name at this directional slot.
                system.cpu.regs[0] = at + 0xc - slot;
            } else {
                system.cpu.regs[0] = at;
                system.cpu.regs[1] = 9; // native self-name lookup
            }
            return;
        }
        at = read_word(system, at + 4);
        if at == record { return; }
    }
}

impl AimMouse {
    pub(super) fn menu_age(&self) -> u64 { 3 + if self.menus_on_top { self.menu_top.duration } else { 0 } }

    pub fn menus_top_dragging(&self) -> bool { self.menu_top.grab.is_some() || self.idol_view.dragging() || self.idol_view.panning() }

    pub fn clear_menus_top_pointer(&mut self) {
        self.menu_top.pointer = None;
        self.menu_top.candidates.clear();
        self.menu_top.hit_frame = None;
    }

    pub fn menus_on_top_shown(&self, frame: u64) -> bool {
        let age = 3 + self.menu_top.duration;
        self.menus_on_top && self.menu_top.drawn.is_some_and(|(drawn, index, object, widen)|
            frame >= drawn && frame <= drawn + age && widen == self.widen
                && if index >= ScreenTransform::scripted_len() {
                    self.menu_screen.is_none_or(|(_, _, seen)| seen <= drawn)
                } else {
                    self.menu_screen.is_some_and(|(active, owner, seen)| active == index && owner == object && frame >= seen
                        && (self.menu_top.passthrough || frame <= seen + age))
                })
    }

    pub(super) fn native_menu_on_top(&self, frame: u64) -> bool {
        self.menus_on_top_shown(frame) && (self.menu_top.passthrough || self.menu_top.drawn.is_some_and(|(_, index, _, _)| index >= ScreenTransform::scripted_len()))
    }

    /// Frontend top-screen pixels (400 across), converted to native touch.
    pub fn menus_top_touch(&mut self, frame: u64, x: f32, y: f32, pressed: bool) -> Option<(f32, f32)> {
        let began = !std::mem::replace(&mut self.idol_view.held, pressed) && pressed;
        if !self.menus_on_top_shown(frame) || self.menu_top.blocked { self.menu_top.grab = None; self.idol_view.cancel(); return None; }
        let point = [x * self.widen, y];
        // An Orbit owns its drag until release, wherever the pointer goes.
        if self.idol_view.dragging() {
            if pressed { self.idol_view.drag(point); return None; }
            self.idol_view.release(self.idol_orbit_gain);
        }
        if !pressed { self.menu_top.grab = None; }
        if self.menu_top.pointer != Some(point) {
            self.menu_top.pointer = Some(point);
            let file = self.menu_top.file_select();
            let cropped = self.menu_screen.is_some_and(|(index, ..)| board_cut(index));
            let on_screen = inside(point, [0.0, 0.0, 400.0 * self.widen, 240.0]);
            let idols = self.menu_screen.is_some_and(|(index, ..)| ScreenTransform::scripted(index).id == super::screen_transform::Screen::Idols);
            self.menu_top.candidates = self.menu_top.parts.iter().enumerate().rev().filter_map(|(index, part)| {
                if !self.menu_top.enabled(part) || part.top || part.idol == Some(IdolGroup::Fountain)
                    || (self.menu_top.idol_dialog && part.idol != Some(IdolGroup::Dialog))
                    || (idols && !idols_takes_pointer(part.model, self.menu_top.idols_list)) { return None; }
                let addressable = part.model.and_then(button_hash).is_some();
                if file && !addressable { return None; }
                let hit = part.hit(point);
                let isolated = addressable && !cropped && on_screen && (!file || part.file_book().is_some());
                if !hit && !isolated { return None; }
                let native = mapped(point, part.place.to, part.place.from);
                if file && !hit && !inside([native.0, native.1], [0.0, 0.0, 320.0, 240.0]) { return None; }
                Some((index, native))
            }).collect();
            self.menu_top.candidate = 0;
        }
        // A press on the Idol View, off every Control, starts an Orbit.
        if began && self.menu_top.grab.is_none() && self.menu_top.candidates.is_empty() && self.idol_view_scale(point).is_some() {
            self.idol_view.press(frame, point);
            self.menu_part = None;
            return None;
        }
        let over = self.menu_top.candidates.get(self.menu_top.candidate).map(|&(index, _)| index);
        let grab = self.menu_top.grab.filter(|_| pressed).map(|grab| {
            // An egg crosses separately placed groups without releasing its native touch.
            if self.menu_top.parts[grab].idol == Some(IdolGroup::Eggs) {
                over.filter(|&index| self.menu_top.parts[index].idol == Some(IdolGroup::Launcher)).unwrap_or(grab)
            } else { grab }
        });
        let Some(index) = grab.or(over) else {
            self.menu_part = None;
            self.menu_looked = None;
            return None;
        };
        let part = &self.menu_top.parts[index];
        let validated = part.model.and_then(button_hash).is_none_or(|expected|
            self.menu_part.is_some_and(|found| found.hash == expected)
                && self.menu_looked.is_some_and(|(point, _)| Some(point) == self.pointer));
        self.menu_top.grab = (pressed && (self.menu_top.grab.is_some() || validated)).then_some(index);
        let native = mapped(point, part.place.to, part.place.from);
        Some(if part.model.and_then(button_hash).is_some() { native } else { touch_point(native) })
    }

    /// On Idols, over the Idol layer's Placement at a top-screen point: the
    /// native pixels a top-screen pixel there spans.
    fn idol_view_scale(&self, point: [f32; 2]) -> Option<[f32; 2]> {
        if !self.menu_screen.is_some_and(|(index, ..)| ScreenTransform::scripted(index).id == super::screen_transform::Screen::Idols) { return None; }
        let part = self.menu_top.parts.iter().find(|part| part.top && part.model == Some(IDOL_VIEW_LAYER)
            && self.menu_top.enabled(part) && inside(point, part.place.to))?;
        Some([part.place.from[2] / part.place.to[2], part.place.from[3] / part.place.to[3]])
    }

    /// what the pointer is doing with the Idol View, for its look: over it
    /// off every Control, Orbiting or Panning it; none elsewhere.
    pub fn idol_view_pointer(&self, frame: u64) -> Option<IdolPointer> {
        if self.idol_view.dragging() { return Some(IdolPointer::Orbit); }
        if self.idol_view.panning() { return Some(IdolPointer::Pan); }
        let point = self.menu_top.pointer.filter(|_| self.menus_on_top_shown(frame) && !self.menu_top.blocked)?;
        (self.menu_top.candidates.is_empty() && self.idol_view_scale(point).is_some()).then_some(IdolPointer::Hover)
    }

    /// A right-button event at a frontend top-screen point (400 across):
    /// a press over the Idol View starts a Pan that holds the pointer until
    /// release. Whether the Pan took it.
    pub fn idol_view_pan(&mut self, frame: u64, x: f32, y: f32, pressed: bool) -> bool {
        let point = [x * self.widen, y];
        if self.idol_view.panning() {
            if pressed { self.idol_view.pan_to(point); } else { self.idol_view.end_pan(); }
            return true;
        }
        if !pressed || !self.menus_on_top_shown(frame) || self.menu_top.blocked || self.idol_view.dragging() { return false; }
        let Some(scale) = self.idol_view_scale(point) else { return false };
        self.idol_view.begin_pan(point, scale);
        true
    }

    /// Wheel steps over the Idol View, positive down, Zoom it; whether the
    /// Idol View took them.
    pub(super) fn idol_view_wheel(&mut self, frame: u64, steps: f32) -> bool {
        let Some(point) = self.menu_top.pointer.filter(|_| self.menus_on_top_shown(frame) && !self.menu_top.blocked) else { return false };
        if self.idol_view_scale(point).is_none() { return false; }
        self.idol_view.zoom_by(-steps * ZOOM_STEP);
        true
    }

    /// The corresponding top-screen point, useful to drive the same
    /// frontend mapping in the headless harness.
    pub fn menus_top_point(&self, frame: u64, x: f32, y: f32, button: Option<u32>) -> Option<(f32, f32)> {
        if !self.menus_on_top_shown(frame) { return None; }
        let part = self.menu_top.parts.iter().rev().find(|part| self.menu_top.enabled(part) && !part.top
            && (!self.menu_top.file_select() || part.model.and_then(button_hash).is_some())
            && (!self.menu_top.idol_dialog || part.idol == Some(IdolGroup::Dialog))
            && (inside([x, y], part.place.from) || button.is_some())
            && button.is_none_or(|button| part.model.and_then(button_hash) == Some(button)))?;
        let [x, y] = part.place.map_point([x, y]);
        Some((x / self.widen, y))
    }

    /// whether the Idols List page, not one idol's description, is shown.
    pub(super) fn idols_list_shown(&self) -> bool { self.menu_top.idols_list }

    pub fn hovered_menu_part(&self) -> Option<u32> { self.menu_part.map(|part| part.hash) }

    /// The chapter of the Chapter Select Row under the pointer, if any.
    pub(super) fn hovered_chapter_row(&self) -> Option<u32> {
        let &(index, _) = self.menu_top.candidates.get(self.menu_top.candidate)?;
        self.menu_top.row_chapter(index)
    }

    /// Scrolls Chapter Select's row by wheel steps, positive right. Only
    /// the view moves: the Outline stays on its chapter.
    pub(super) fn scroll_chapters(&mut self, steps: f32) {
        let menu_top = &mut self.menu_top;
        menu_top.chapter_wheel += steps;
        let whole = menu_top.chapter_wheel.trunc();
        menu_top.chapter_wheel -= whole;
        let (count, fit) = menu_top.chapter_span;
        menu_top.chapter_scroll = (menu_top.chapter_scroll as f32 + whole).clamp(0.0, count.saturating_sub(fit) as f32) as usize;
    }

    /// the edited Screen's layers: label and target box, in pixels as shown.
    /// the Cauldron's parts, the active Scripted Menu's, or none on Pause.
    pub fn editor_layers(&self) -> Vec<(String, [f32; 4])> {
        if self.editing_cauldron() { return self.cauldron_editor_layers(); }
        if !self.editing_scripted() { return Vec::new(); }
        self.menu_top.parts.iter().enumerate().map(|(index, part)| {
            let name = if self.menu_top.file_select() {
                match index {
                    0 => "Book I", 1 => "Book II", 2 => "Book III", 3 => "Back",
                    4 => "How to Play", 5 => "HOME hint", 6 => "Player information Board", 7 => "Bottom-screen background",
                    8 => "Paper background", 9 => "Coloured ribbons", 10 => "Top-left decoration",
                    11 => "Bottom-left decoration", 12 => "Bottom-right decoration", 13 => "Right-edge decoration",
                    14 => "Feather decoration", _ => "Coloured crest",
                }.to_owned()
            } else if part.backdrop {
                let screen = self.menu_screen.map(|(screen, ..)| screen).or_else(|| self.menu_top.drawn.map(|(_, screen, ..)| screen));
                if screen.is_some_and(details_of) { "Details card" } else { "Top screen" }.to_owned()
            } else if let Some(chapter) = self.menu_top.row_chapter(index) {
                format!("Chapter {chapter}")
            } else if part.chapter {
                CHAPTER_LAYERS.get(index).map_or("controls", |&(name, ..)| name).to_owned()
            } else if self.menu_screen.is_some_and(|(screen, ..)| screen == 11) && !self.menu_top.idol_dialog {
                match index { 0 => "Dialog", 1 => "No", 2 => "Yes", _ => "hint" }.to_owned()
            } else if let Some(group) = part.idol {
                group.name().to_owned()
            } else { part.model.map_or_else(|| if part.top { "hint" } else { "controls" }.to_owned(), str::to_owned) };
            let inactive = if self.menu_top.available(part) { "" } else { " (inactive)" };
            (format!("{name} #{index}{inactive}"), part.place.to)
        }).collect()
    }

    /// the Scripted Menu the editor's layers belong to, by registry index;
    /// none for the Cauldron, Pause and the Retreat Dialog.
    pub fn editor_scripted_index(&self) -> Option<usize> {
        if self.editing_cauldron() || !self.editing_scripted() { return None; }
        self.menu_screen.map(|(screen, ..)| screen).or_else(|| self.menu_top.drawn.map(|(_, screen, ..)| screen))
    }

    /// the dev editor's saved moves, by screen and part index.
    pub fn editor_overrides(&self) -> Vec<(usize, usize, [f32; 4])> {
        self.menu_top.overrides.clone()
    }

    pub fn set_editor_overrides(&mut self, overrides: Vec<(usize, usize, [f32; 4])>) {
        self.menu_top.overrides = overrides;
    }

    /// Visibility follows the same stable indices as `editor_layers`.
    pub fn editor_layer_visibility(&self) -> Vec<bool> {
        if self.editing_cauldron() { return self.cauldron_editor_visibility(); }
        if !self.editing_scripted() { return Vec::new(); }
        self.menu_top.parts.iter().map(|part| part.visible).collect()
    }

    pub fn editor_hidden_layers(&self) -> Vec<(usize, usize)> {
        self.menu_top.hidden.clone()
    }

    pub fn set_editor_hidden_layers(&mut self, hidden: Vec<(usize, usize)>) {
        self.menu_top.hidden = hidden;
        if let Some(screen) = self.menu_screen.map(|(screen, ..)| screen)
            .or_else(|| self.menu_top.drawn.map(|(_, screen, ..)| screen)) {
            for (index, part) in self.menu_top.parts.iter_mut().enumerate() {
                part.visible = !self.menu_top.hidden.contains(&(screen, index));
            }
        }
        self.cancel_menu_touch();
        self.clear_menus_top_pointer();
    }

    pub fn set_editor_layer_visible(&mut self, index: usize, visible: bool) {
        if self.editing_cauldron() { return self.set_cauldron_editor_visible(index, visible); }
        if !self.editing_scripted() { return; }
        let Some(screen) = self.menu_screen.map(|(screen, ..)| screen)
            .or_else(|| self.menu_top.drawn.map(|(_, screen, ..)| screen)) else { return };
        let Some(part) = self.menu_top.parts.get_mut(index) else { return };
        part.visible = visible;
        self.menu_top.hidden.retain(|&entry| entry != (screen, index));
        if !visible { self.menu_top.hidden.push((screen, index)); }
        self.cancel_menu_touch();
        self.clear_menus_top_pointer();
    }

    /// move one layer of the edited Screen, keeping the edit for the next
    /// frame.
    pub fn set_editor_layer(&mut self, index: usize, to: [f32; 4]) {
        if self.editing_cauldron() { return self.set_cauldron_editor_layer(index, to); }
        if !self.editing_scripted() { return; }
        let screen = self.menu_screen.map(|(screen, ..)| screen).or_else(|| self.menu_top.drawn.map(|(_, screen, ..)| screen));
        // A texture layer rides on the layer drawing its texture: moving
        // that owner moves and scales it the same way.
        if let (Some(screen), Some(owner)) = (screen, self.menu_top.parts.get(index).cloned()) {
            let owned_by = |model: &str| top_layers(screen).iter().find(|&&(_, owns, _)| owns == model).map(|&(layer, ..)| layer);
            let riders: Vec<(usize, [f32; 4])> = texture_layers(screen).iter()
                .filter(|&&(_, model, _)| match owned_by(model) { Some(layer) => owner.model == Some(layer), None => owner.backdrop })
                .filter_map(|&(label, ..)| self.menu_top.parts.iter().position(|part| part.model == Some(label)))
                .map(|rider| {
                    let moved = Placement { from: owner.place.to, to };
                    (rider, moved.to_box(self.menu_top.parts[rider].place.to))
                }).collect();
            if let Some(part) = self.menu_top.parts.get_mut(index) { part.place.to = to; }
            for (rider, box_) in riders { self.set_editor_layer(rider, box_); }
        }
        // A texture layer stays where its texture can draw.
        let to = match screen.and_then(|screen| Some((screen, texture_layer(screen, self.menu_top.parts.get(index)?.model)?))) {
            Some((screen, (_, owner, native))) => texture_owner(screen, &self.menu_top.parts, owner)
                .map_or(to, |place| clamp_texture_box(to, native, place)),
            None => to,
        };
        if let Some(part) = self.menu_top.parts.get_mut(index) {
            part.place.to = to;
        }
        self.clear_menus_top_pointer();
        let Some(screen) = screen else { return };
        // A widened Counters box (idol name bar) is saved at its normal width.
        let mut to = to;
        if let Some(part) = self.menu_top.parts.get(index).filter(|part| part.idol == Some(IdolGroup::Counters)) {
            to[2] *= IDOL_COUNTERS_WIDTH / part.place.from[2];
        }
        let kept = unseat_override(to, 400.0 * self.widen);
        match self.menu_top.overrides.iter_mut().find(|(kept, part, _)| *kept == screen && *part == index) {
            Some(entry) => entry.2 = kept,
            None => self.menu_top.overrides.push((screen, index, kept)),
        }
    }

    pub fn widen(&self) -> f32 {
        self.widen
    }

    /// Host-only pointers and recorded drawings must never survive loading
    /// another console state. Enhancement settings stay with the host.
    pub(crate) fn reset_menu_after_load(&mut self) {
        self.cancel_menu_touch();
        self.menu_top = MenuTop {
            blocked: self.menu_top.blocked,
            overrides: std::mem::take(&mut self.menu_top.overrides),
            hidden: std::mem::take(&mut self.menu_top.hidden),
            ..MenuTop::default()
        };
        self.pointer = None;
        self.menu_screen = None;
        self.menus_open.clear();
        self.menu_looked = None;
        self.menu_part = None;
        self.menu_calls.clear();
        self.menu_touch = None;
        self.menu_touched = None;
        self.bottom_scene = None;
        self.saved = None;
        self.top_viewport = None;
        self.top_projection = None;
        self.placed.clear();
        self.top_moved.clear();
        self.transform.reset();
        self.battle_drawn = None;
    }
}

/// Transparent portions of overlapping button boxes must not steal the
/// pointer. Try the next box when the game's own hit test disagrees.
pub(super) fn refresh_hover(mouse: &mut AimMouse, frame: u64) {
    // Chapter selection scrolls its list as it outlines a thumbnail. Keep
    // that native selection until the pointer moves; re-querying every
    // refresh clears the completed hit before a click can consume it.
    if mouse.menu_top.drawn.is_some_and(|(_, index, _, _)| index == 1) { return; }
    // A hit query can span a refresh. Finish its candidate chain before
    // starting another query for the same stationary host pointer.
    if std::mem::take(&mut mouse.menu_top.retrying_hit) { return; }
    // An Orbit holds the button down; this re-query would read as a release.
    if mouse.menu_top.hit_frame == Some(frame) || !mouse.mouse_mode || mouse.menu_top.grab.is_some()
        || mouse.idol_view.dragging() || mouse.idol_view.panning() || mouse.menu_click || !mouse.menus_on_top_shown(frame) { return; }
    mouse.menu_top.hit_frame = Some(frame);
    if let Some(point) = mouse.menu_top.pointer {
        mouse.menu_top.pointer = None;
        let pointer = mouse.menus_top_touch(frame, point[0] / mouse.widen, point[1], false);
        mouse.pointer = pointer;
        mouse.menu_looked = None;
    }
}

pub(super) fn accept_hit(system: &mut System, found: Option<u32>) -> bool {
    let Some(mouse) = system.aim_mouse.as_mut() else { return true };
    let Some(&(index, point)) = mouse.menu_top.candidates.get(mouse.menu_top.candidate) else { return true };
    let point = if mouse.menu_top.parts[index].model.and_then(button_hash).is_some() { point } else { touch_point(point) };
    if mouse.pointer != Some(point) || mouse.menu_top.grab.is_some() { return true; }
    let Some(expected) = mouse.menu_top.parts[index].model.and_then(button_hash) else { return true };
    log::debug!("menu hit {:?} at {point:?}: expected {expected:08X}, found {found:08X?}", mouse.menu_top.parts[index].model);
    if found == Some(expected) { return true; }
    mouse.menu_top.candidate += 1;
    mouse.menu_top.retrying_hit = true;
    mouse.pointer = mouse.menu_top.candidates.get(mouse.menu_top.candidate).map(|&(index, point)|
        if mouse.menu_top.parts[index].model.and_then(button_hash).is_some() { point } else { touch_point(point) });
    mouse.menu_looked = None;
    mouse.menu_part = None;
    if mouse.pointer.is_none() { mouse.menu_click = false; }
    false
}

fn scene(system: &mut System, top: u32) -> Option<(usize, u32, u32, bool)> {
    let mouse = system.aim_mouse.as_ref()?;
    if !mouse.menus_on_top || mouse.cauldron_shown(system.frames) { return None; }
    let bottom = mouse.bottom_scene?;
    let active = mouse.menu_screen.filter(|&(_, _, seen)| system.frames >= seen && system.frames <= seen + 3 + mouse.menu_top.duration);
    let last = mouse.menu_screen;
    let rendering = mouse.menu_top.active;
    let current = read_word(system, top + 0x40);
    if read_word(system, bottom) != BOTTOM_SCENE || current == 0 || read_word(system, bottom + 0x40) != current { return None; }
    if let Some((index, object, _, native)) = rendering.filter(|&(_, _, scene, _)| scene == current) { return Some((index, object, bottom, native)); }
    // The game's own MENUID changes before a menu's first drawn frame, while
    // the last Screen to take input can still be the one it left.
    let game = current_menu(system);
    let agrees = |index: usize| game.is_none_or(|id| menus_of(index).is_empty() || menus_of(index).contains(&id));
    if let Some((index, object, _)) = active.filter(|&(index, ..)| agrees(index)) { return Some((index, object, bottom, false)); }
    if let Some(index) = game.and_then(screen_of) { return Some((index, 0, bottom, false)); }
    // Models are present during entrance and exit animations before the
    // corresponding input update runs. Draw their layout without enabling input.
    let layers = read_word(system, current + SCENE_LAYERS);
    let mut matched = None;
    let mut fallback = None;
    for layer in BOTTOM_LAYERS {
        let list = read_word(system, layers + layer * LAYER_SIZE + 4);
        if list == 0 { continue; }
        let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
        if end < start || end - start > 0x1000 { continue; }
        for at in (start..end).step_by(4) {
            let model = read_word(system, at);
            let name = model_name(system, model);
            let hub = if name.starts_with("MenSgHomeBt") { Some(12) }
                else if name.starts_with("MenTopBt") { Some(2) }
                else if name.starts_with("MenAlterBt") { Some(13) }
                else if name.starts_with("MenTrymissionBt") { Some(21) }
                else if name.starts_with("MenCollectionTopBt") { Some(24) }
                else if name.starts_with("MenOptionOtherCaptionBt") { Some(7) }
                else if name.starts_with("MenOptionOtherBt") { Some(4) }
                else if name.starts_with("MenOptionControlBt") { Some(5) }
                else if name.starts_with("MenOptionSoundBt") { Some(8) }
                else if name.starts_with("MenOptionNameBt") { Some(9) }
                else if name.starts_with("MenOptionCtlTpenBt") { Some(6) }
                else if name.starts_with("MenSelnameBt") { Some(10) }
                else if name.starts_with("MenSelfileBt") { Some(0) }
                else if name.starts_with("MenStageselectBt") { Some(1) }
                else if name.starts_with("MenEquipBt0001") { Some(14) }
                else if name.starts_with("MenEquipBt0211") { Some(18) }
                else if name.starts_with("MenEquipBt0120") { Some(19) }
                else if name.starts_with("MenEquipBt0102") { Some(20) }
                else if name.starts_with("MenOptionTopBt") { Some(3) }
                else { None };
            if let Some(index) = hub {
                matched = Some(index);
                continue;
            }
            let kind = if name.starts_with("MenOption") { 0 }
                else if name.starts_with("MenColSdtstMusic") { 1 }
                else if name.starts_with("MenColVision") { 6 }
                else if name.starts_with("MenCol") { 2 }
                else if name.starts_with("MenEquipBt02") { 3 }
                else if name.starts_with("MenAltermix") { 4 }
                else if name.starts_with("MenStockroom") { 5 }
                else if name.starts_with("MenEquip") || name.starts_with("MenAlterbuy") || name.starts_with("MenAltersold") { 7 }
                else if name.starts_with("MenSel") || name.starts_with("MenStageselect") { 8 }
                else { continue };
            if let Some((index, object, _)) = last.filter(|&(index, _, _)| matches!((kind, index),
                (3, 19) | (4, 17) | (5, 23) | (6, 22 | 25) | (7, 14..=20))) {
                fallback.get_or_insert((index, object, bottom, true));
            } else {
                fallback.get_or_insert((ScreenTransform::scripted_len() + kind, current, bottom, true));
            }
        }
    }
    // Solo's bag loads before its bottom Controls during entry.
    if matched.is_none() && fallback.is_none() {
        for layer in 0..16 {
            let list = read_word(system, layers + layer * LAYER_SIZE + 4);
            if list == 0 { continue; }
            let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
            if end < start || end - start > 0x1000 { continue; }
            if (start..end).step_by(4).any(|at| {
                let model = read_word(system, at);
                model_name(system, model) == "MenCmnUpHeart0000"
            }) {
                matched = (0..ScreenTransform::scripted_len()).find(|&index| ScreenTransform::scripted(index).layout.heart());
                break;
            }
        }
    }
    matched.map(|index| (index, 0, bottom, false)).or(fallback)
}

pub(super) fn begin(system: &mut System) -> bool {
    if let Some(back) = system.aim_mouse.as_mut().and_then(|mouse| mouse.saved.take_if(|saved| saved.drawing == Drawing::MenuClip(START))) {
        system.cpu.regs = back.regs;
        system.cpu.cpsr = back.cpsr;
        system.cpu.vfp.regs = back.vfp;
        return true;
    }
    let top = system.cpu.regs[4];
    let Some((index, object, bottom, native_touch)) = scene(system, top) else {
        if let Some(mouse) = system.aim_mouse.as_mut().filter(|mouse| mouse.saved.is_none()) { mouse.menu_top.drawn = None; }
        return false;
    };
    let current = read_word(system, top + 0x40);
    let idol_dialog = matches!(index, 10 | 11) && scene_has_idol(system, current);
    let controls_more = index == 5 && read_word(system, object + 0x11C) & 0x100 != 0;
    let idol_revealing = index < ScreenTransform::scripted_len()
        && matches!(ScreenTransform::scripted(index).layout, LayoutKind::Idol)
        && idol_revealing(system, object);
    let chapters = if index == 1 { chapter_list(system, bottom) } else { Vec::new() };
    let outlined = (index == 1 && object != 0).then(|| read_word(system, object + CHAPTER) + 1);
    log::debug!("menu begin index {index} idol dialog {idol_dialog}");
    // the native fallback screens past the registry draw themselves and have
    // no Screen row to cap
    let screen = (index < ScreenTransform::scripted_len()).then(|| ScreenTransform::scripted(index).id);
    let mouse = system.aim_mouse.as_mut().unwrap();
    if mouse.saved.is_some() { return false; }
    if let Some(screen) = screen {
        mouse.transform.mark_active(screen, system.frames);
    }
    if mouse.menu_top.drawn.is_some_and(|(_, old, owner, widen)| old != index || owner != object || widen != mouse.widen) {
        mouse.cancel_menu_touch();
        mouse.menu_looked = None;
        mouse.menu_part = None;
        mouse.menu_top.pointer = None;
        mouse.menu_top.candidates.clear();
        mouse.menu_top.chapter_scroll = 0;
        mouse.menu_top.chapter_wheel = 0.0;
        mouse.menu_top.chapter_outlined = None;
    }
    let (mut native, mut parts) = layout_with_dialog(index, 400.0 * mouse.widen, idol_dialog);
    for &(screen, part, to) in &mouse.menu_top.overrides {
        if screen == index {
            if let Some(part) = parts.get_mut(part) {
                part.place.to = seat_override(to, 400.0 * mouse.widen);
            }
        }
    }
    for &(screen, part) in &mouse.menu_top.hidden {
        if screen == index {
            if let Some(part) = parts.get_mut(part) { part.visible = false; }
        }
    }
    // The revealed idol's name bar draws with the counters but runs past
    // their 160-pixel source; widen the box at the same scale meanwhile.
    if idol_revealing {
        for part in parts.iter_mut().filter(|part| part.idol == Some(IdolGroup::Counters)) {
            part.place.to[2] *= IDOL_NAME_WIDTH / part.place.from[2];
            part.place.from[2] = IDOL_NAME_WIDTH;
        }
    }
    let menu_top = &mut mouse.menu_top;
    menu_top.chapter_rows.clear();
    if index == 1 {
        let width = 400.0 * mouse.widen;
        let count = chapters.len();
        let fit = chapter_slots(width, 0, 0).1;
        // Keys pull the row to a newly outlined chapter; the wheel never
        // moves the Outline, so it may scroll it out of view.
        if outlined != menu_top.chapter_outlined {
            menu_top.chapter_outlined = outlined;
            if let Some(row) = outlined.and_then(|number| chapters.iter().position(|&(chapter, _)| chapter == number)) {
                if row < menu_top.chapter_scroll { menu_top.chapter_scroll = row; }
                if row >= menu_top.chapter_scroll + fit { menu_top.chapter_scroll = row + 1 - fit; }
            }
        }
        menu_top.chapter_scroll = menu_top.chapter_scroll.min(count.saturating_sub(fit));
        menu_top.chapter_span = (count, fit);
        for (row, to) in chapter_slots(width, count, menu_top.chapter_scroll).0 {
            let (number, [x, y]) = chapters[row];
            let [w, h] = CHAPTER_ROW_SOURCE;
            let from = [x - w * 0.5, y - h * 0.5, w, h];
            parts.push(Part { visible: true, backdrop: false, chapter: true, idol: None, top: false, model: None, place: Placement { from, to } });
            menu_top.chapter_rows.push(number);
        }
    }
    // The game's own top drawing follows its editable backdrop layer.
    let backdrop = parts.iter().find(|part| part.backdrop).map(|part| (part.place.to, part.visible));
    if let Some((to, _)) = backdrop { native.to = to; }
    mouse.menu_top.idol_dialog = idol_dialog;
    mouse.menu_top.idol_revealing = idol_revealing;
    mouse.menu_top.controls_more = controls_more;
    mouse.menu_top.active = Some((index, object, current, native_touch));
    mouse.menu_top.passthrough = native_touch;
    mouse.menu_top.started = system.frames;
    mouse.menu_top.parts = parts;
    let order = if mouse.menu_top.file_select() {
        // Background and information first; interactive Controls above them.
        [7, 8, 9, 10, 11, 12, 13, 14, 15, 6, 0, 1, 2, 3, 4, 5].into_iter().filter(|&index|
            mouse.menu_top.enabled(&mouse.menu_top.parts[index])).collect()
    } else {
        let parts = mouse.menu_top.parts.clone();
        let mut order = draw_order(system, bottom, &parts, !top_layers(index).is_empty(), bottom_layers(index));
        // A texture layer draws through its owner's mesh, never by itself.
        order.retain(|&part| texture_layer(index, parts[part].model).is_none());
        order
    };
    system.aim_mouse.as_mut().unwrap().menu_top.draw_order = order;
    let idols_list = !bottom_layers(index).is_empty() && {
        let found = std::cell::Cell::new(false);
        filter_models(system, bottom, false, false, &mut Vec::new(), |name| { found.set(found.get() || name == "MenColViewlistBt0000"); true });
        found.get()
    };
    system.aim_mouse.as_mut().unwrap().menu_top.idols_list = idols_list;
    let mut moved = Vec::new();
    place_scene(system, top, top, native, true, false, &mut moved);
    if system.aim_mouse.as_ref().unwrap().menu_top.file_select() {
        file_models(system, top, None, &mut moved);
    }
    if index == 12 { filter_models(system, top, true, false, &mut moved, |name| name != "MenCmnUpHeart0000"); }
    let trace_models = std::env::var_os("KIUTRACE_MENU_MODELS").is_some();
    if trace_models || !top_layers(index).is_empty() {
        // The split layers draw these nodes; the Details card keeps the rest.
        // KIUTRACE_MENU_MODELS logs every top-screen model and its nodes,
        // the input for splitting a new Details card.
        let scene = read_word(system, top + 0x40);
        let layers = read_word(system, scene + SCENE_LAYERS);
        for layer in 0..0x13 {
            let list = read_word(system, layers + layer * LAYER_SIZE + 4);
            if list == 0 { continue; }
            let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
            if end < start || end - start > 0x1000 { continue; }
            for at in (start..end).step_by(4) {
                let model = read_word(system, at);
                let name = model_name(system, model);
                if trace_models { log_model_nodes(system, layer, model, &name); }
                if layer >= 0x10 { continue; }
                if top_layers(index).is_empty() { continue; }
                show_parts(system, &mut moved, model, |node| top_layer(top_layers(index), &name, node).is_none());
            }
        }
    }
    if index == 1 { chapter_models(system, top, true, false, None, &mut moved); }
    place_textures(system, index, top);
    let mouse = system.aim_mouse.as_mut().unwrap();
    mouse.menu_top.native = moved;
    // A hidden backdrop keeps the game's pass but clips all of it away.
    let clip = if !top_layers(index).is_empty() || backdrop.is_some_and(|(_, visible)| !visible) { Placement { from: native.from, to: [0.0; 4] } } else { native };
    mouse.menu_top.clip = Some((clip, true));
    set_clip(system, Drawing::MenuClip(START), Vec::new());
    true
}

fn place_scene(system: &mut System, top: u32, drawing: u32, place: Placement, top_camera: bool, isolated: bool, moved: &mut Vec<(u32, Vec<u8>)>) {
    let scale = [place.to[2] / place.from[2], place.to[3] / place.from[3]];
    let size = [if top_camera { 400.0 } else { 320.0 }, 240.0];
    let full = if isolated { place.to } else { [place.to[0] - place.from[0] * scale[0], place.to[1] - place.from[1] * scale[1], size[0] * scale[0], size[1] * scale[1]] };
    let viewport = viewport_bytes(system, top, full);
    change(system, moved, drawing + 0x44, &viewport);
    let scene = read_word(system, drawing + 0x40);
    let screen = read_word(system, drawing + 0x58);
    let camera = read_word(system, scene + SCENE_CAMERAS + screen * 4);
    crop_camera(system, camera, if isolated { place.from } else { [0.0, 0.0, size[0], size[1]] }, size, top_camera, moved);
}

fn filter_models(system: &mut System, drawing: u32, top: bool, foreground: bool, moved: &mut Vec<(u32, Vec<u8>)>, keep: impl Fn(&str) -> bool) {
    let scene = read_word(system, drawing + 0x40);
    let layers = read_word(system, scene + SCENE_LAYERS);
    for layer in if top { 0..16 } else { BOTTOM_LAYERS } {
        let list = read_word(system, layers + layer * LAYER_SIZE + 4);
        if list == 0 { continue; }
        let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
        if end < start || end - start > 0x1000 { continue; }
        let models: Vec<u32> = (start..end).step_by(4).map(|at| read_word(system, at)).collect();
        let kept: Vec<u32> = models.iter().copied().filter(|&model| keep(&model_name(system, model))).collect();
        if foreground { for &model in &kept { foreground_materials(system, model, moved); } }
        if kept.len() != models.len() {
            let bytes: Vec<u8> = kept.iter().flat_map(|model| model.to_le_bytes()).collect();
            change(system, moved, start, &bytes);
            change(system, moved, list + 0x1C, &(start + bytes.len() as u32).to_le_bytes());
        }
    }
}

// File Select's books are 3D meshes: keep their native depth and material
// ordering. The 2D foreground override flattens their covers behind other faces.
// Book meshes use the full shown viewport and a placed projection. Font
// ownership is checked in native coordinates, not projected coordinates.
fn file_models(system: &mut System, drawing: u32, part: Option<&Part>, moved: &mut Vec<(u32, Vec<u8>)>) {
    let scene = read_word(system, drawing + 0x40);
    let layers = read_word(system, scene + SCENE_LAYERS);
    for layer in 0..16 {
        let list = read_word(system, layers + layer * LAYER_SIZE + 4);
        if list == 0 { continue; }
        let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
        if end < start || end - start > 0x1000 { continue; }
        for at in (start..end).step_by(4) {
            let model = read_word(system, at);
            let name = model_name(system, model);
            let selected = part.is_some_and(|part| if part.top {
                name == "MenSelFileUp0001"
            } else { name == part.model.unwrap_or("MenDemoBt0000") });
            // The upper model also contains the backdrop and corner artwork.
            show_nodes(system, moved, model, |node| selected && part.is_some_and(|part| if part.top {
                match part.model {
                    None => !node.starts_with("NN_U") && node != "fade" && !node.starts_with("RotObj"),
                    Some("NN_U0000") => matches!(node, "NN_U0000" | "NN_U0001" | "NN_U0002" | "fade"),
                    Some("NN_U0004") => matches!(node, "NN_U0004" | "NN_U0010"),
                    Some(model) => node == model,
                }
            } else { !node.starts_with("RotObj") }));
        }
    }
}

/// What one Chapter Select Row draws: its chapter, the thumbnail's center
/// on the bottom screen, and whether the game outlines it.
#[derive(Clone, Copy)]
struct RowDraw {
    chapter: u32,
    center: [f32; 2],
    outlined: bool,
}

/// A model's world position (its 3x4 matrix's translation).
const MODEL_WORLD_X: u32 = 0x98;
const MODEL_WORLD_Y: u32 = 0xA8;

// Chapter scenery and information share a model. Select mesh nodes so
// the scenery can fill the screen while the complete information shrinks.
// On the bottom, a Row draws only its own chapter's thumbnail, laurel and,
// when outlined, the cursor: replaying the whole List in every Row runs
// out of the game's drawing buffer by the fourth. The controls draw the rest.
fn chapter_models(system: &mut System, drawing: u32, top: bool, details: bool, row: Option<RowDraw>, moved: &mut Vec<(u32, Vec<u8>)>) {
    let scene = read_word(system, drawing + 0x40);
    let layers = read_word(system, scene + SCENE_LAYERS);
    let thumbnail = row.map(|row| format!("ML_B{:04}", row.chapter));
    for layer in if top { 0..16 } else { BOTTOM_LAYERS } {
        let list = read_word(system, layers + layer * LAYER_SIZE + 4);
        if list == 0 { continue; }
        let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
        if end < start || end - start > 0x1000 { continue; }
        for at in (start..end).step_by(4) {
            let model = read_word(system, at);
            let name = model_name(system, model);
            if std::env::var_os("KIUTRACE_MENU_MODELS").is_some() {
                log_model_nodes(system, layer, model, &name);
            }
            let selected = if top { name.starts_with("MenStageselectUp") } else if let Some(row) = row {
                match name.as_str() {
                    "MenStageselectBt0000" => true,
                    "MenStageselectBt0003" => row.outlined,
                    // one laurel per chapter, kept where its chapter is
                    "MenStageselectBt0004" => {
                        let [x, y] = [MODEL_WORLD_X, MODEL_WORLD_Y].map(|at| f32::from_bits(read_word(system, model + at)));
                        let [cx, cy] = chapter_center(x, y);
                        (cx - row.center[0]).abs() < 30.0 && (cy - row.center[1]).abs() < 30.0
                    }
                    _ => false,
                }
            } else { name == "MenStageselectBt9999" };
            if selected && (details || !top) { foreground_materials(system, model, moved); }
            show_nodes(system, moved, model, |node| selected && if top {
                let background = name == "MenStageselectUp0000" && node != "PN_U0002";
                background != details
            } else { name != "MenStageselectBt0000" || thumbnail.as_deref().is_some_and(|thumbnail| node.starts_with(thumbnail)) });
        }
    }
}

/// For the harness: every mesh node of a scene model, with whether it shows.
fn log_model_nodes(system: &mut System, layer: u32, model: u32, name: &str) {
    let cmdl = read_word(system, model + 8);
    let dictionary = read_word(system, cmdl + 0xD0);
    let nodes = read_word(system, model + MODEL_NODES);
    let mut list = Vec::new();
    if dictionary != 0 && nodes != 0 {
        let dictionary = cmdl + 0xD0 + dictionary;
        let count = read_word(system, dictionary + 8).min(read_word(system, model + MODEL_NODES + 4).wrapping_sub(nodes) / 8);
        for node in 0..count {
            let at = dictionary + 0x24 + node * 0x10;
            let at = at.wrapping_add(read_word(system, at));
            let shown = read_word(system, nodes + node * 8 + 4) != 0;
            list.push(format!("{}{}", c_string(system, at), if shown { "" } else { " (hidden)" }));
        }
    }
    log::info!("model layer {layer:X} {model:08X} {name}: {}", list.join(", "));
    for (index, mesh) in super::model_meshes(system, model).into_iter().enumerate() {
        let (visible, node) = (read_word(system, mesh + 0x24) & 0xFF, read_word(system, mesh + 0x26) as i16);
        log::info!("  mesh {index} at {mesh:08X} visible {visible} node {node}");
    }
}

fn foreground_materials(system: &mut System, model: u32, moved: &mut Vec<(u32, Vec<u8>)>) {
    if read_word(system, model) != 0x00D14AF4 { return; }
    let (start, end) = (read_word(system, model + 0x164), read_word(system, model + 0x168));
    if start == 0 || end < start || end - start > 128 * 4 || !system.memory.is_readable(start, end - start) { return; }
    for at in (start..end).step_by(4) {
        let material = read_word(system, at);
        if !system.memory.is_readable(material, 0x18) || read_word(system, material) != 0x00D1510C { continue; }
        // Animated material properties can come from either the original
        // MTOB or its runtime copy. Restore both after this replay.
        for offset in [8, 0x14] {
            let material = read_word(system, material + offset);
            if material == 0 || !system.memory.is_readable(material, 0x124) || read_word(system, material + 4) != u32::from_le_bytes(*b"MTOB")
                || read_word(system, material + 0x120) != 0x0001_0107 { continue; }
            change(system, moved, material + 0x118, &[0; 4]);
            let depth = read_word(system, material + 0x11C) & !1;
            change(system, moved, material + 0x11C, &depth.to_le_bytes());
        }
    }
}

fn set_clip(system: &mut System, drawing: Drawing, moved: Vec<(u32, Vec<u8>)>) {
    let manager = read_word(system, VSYNC_MANAGER);
    let enabled = system.aim_mouse.as_ref().unwrap().menu_top.clip.is_some();
    call_here(system, SCISSOR, manager, u32::from(enabled), drawing, moved);
    let sp = system.cpu.regs[13] - 0x20;
    system.memory.write_bytes(sp, &[0; 16]);
    system.cpu.regs[2] = sp;
    system.cpu.regs[3] = 0;
    system.cpu.regs[13] = sp;
    // call_here executes the callee's first instruction immediately, so
    // injected calls do not visit its entry hook on the next step.
    if enabled { scissor(system); }
}

pub(super) fn scissor(system: &mut System) {
    let Some(mouse) = system.aim_mouse.as_ref() else { return };
    let Some((place, top)) = mouse.menu_top.clip else { return };
    let widen = mouse.widen;
    let text_clip = mouse.menu_top.text_parent.is_some();
    // A Row draws only its own chapter, so it needs no cut at all: not its
    // slot, which would crop its laurel and cursor, nor the native List
    // window, which hides a chapter the game scrolled off its screen.
    let row = matches!(drawing(system), Some(Drawing::MenuTop(step))
        if mouse.menu_top.part_index(step / 8).and_then(|index| mouse.menu_top.row_chapter(index)).is_some());
    let own = text_clip || matches!(drawing(system), Some(Drawing::MenuClip(_)))
        || matches!(drawing(system), Some(Drawing::MenuTop(step)) if step % 8 == 0);
    let at = system.cpu.regs[2];
    let mut old = [0; 16];
    system.memory.read_bytes(at, &mut old);
    // An isolated hub model can animate outside its nominal hit box.
    // Only grouped lists/boards need their native scrolling clips.
    let isolated = if text_clip { !mouse.menu_top.drawing_text && mouse.menu_top.text_parent.as_ref().is_some_and(|saved| !matches!(saved.drawing, Drawing::MenuClip(_))) } else if mouse.menu_top.drawing_text { false } else if row { true } else { match drawing(system) {
        Some(Drawing::MenuTop(step)) => (!mouse.menu_top.file_select()
            || mouse.menu_top.part(step / 8).is_some_and(|part| part.file_book().is_some()))
            && mouse.menu_top.part(step / 8).is_some_and(|part| part.idol.is_some_and(IdolGroup::uncut)
                || part.model.is_some() && !mouse.menu_top.active.is_some_and(|(index, ..)| part_cut(index, part.model))),
        _ => false,
    } };
    let mut rect = if isolated { [0.0, 0.0, 400.0 * widen, 240.0] } else { place.to };
    if let Some(Drawing::MenuTop(step)) = drawing(system) {
        if let Some(part) = mouse.menu_top.part(step / 8).filter(|part| part.file_book().is_some()) {
            // The model also contains its open, upper-screen representation
            // far outside the native Board. Allow a whole book of animation
            // beyond each Board edge, but do not replay that other view.
            let [width, height] = [part.place.from[2], part.place.from[3]];
            let start = part.place.map_point([-width, -height]);
            let end = part.place.map_point([320.0 + width, 240.0 + height]);
            let right = (rect[0] + rect[2]).min(end[0]);
            let bottom = (rect[1] + rect[3]).min(end[1]);
            rect[0] = rect[0].max(start[0]);
            rect[1] = rect[1].max(start[1]);
            rect[2] = (right - rect[0]).max(0.0);
            rect[3] = (bottom - rect[1]).max(0.0);
        }
    }
    if !isolated && !own && system.cpu.regs[1] != 0 {
        let raw: Vec<f32> = old.as_chunks::<4>().0.iter().map(|word| i32::from_le_bytes(*word) as f32).collect();
        let width = if top { 400.0 } else { 320.0 };
        let from = [width - raw[1] - raw[3], 240.0 - raw[0] - raw[2]];
        let (x, y) = mapped(from, place.from, place.to);
        let end = [x + raw[3] * place.to[2] / place.from[2], y + raw[2] * place.to[3] / place.from[3]];
        rect[0] = rect[0].max(x);
        rect[1] = rect[1].max(y);
        rect[2] = (place.to[0] + place.to[2]).min(end[0]) - rect[0];
        rect[3] = (place.to[1] + place.to[3]).min(end[1]) - rect[1];
    }
    let clip = [240.0 - rect[1] - rect[3], 400.0 - (rect[0] + rect[2]) / widen, rect[3].max(0.0), (rect[2] / widen).max(0.0)];
    let bytes: Vec<u8> = clip.into_iter().flat_map(|value| (value.round() as i32).to_le_bytes()).collect();
    log::debug!("menu scissor {:?}, own {own}, rect {rect:?}, rotated {clip:?}", drawing(system));
    system.memory.write_bytes(at, &bytes);
    system.cpu.regs[1] = 1;
    system.cpu.regs[3] = 0;
    system.aim_mouse.as_mut().unwrap().menu_top.scissor_back = Some((at, old));
}

pub(super) fn scissor_done(system: &mut System) {
    if let Some((at, old)) = system.aim_mouse.as_mut().and_then(|mouse| mouse.menu_top.scissor_back.take()) {
        system.memory.write_bytes(at, &old);
    }
}

pub(super) fn text(system: &mut System, place: u32) -> bool {
    if drawing(system) == Some(Drawing::Viewport(place)) && system.aim_mouse.as_ref().is_some_and(|mouse| mouse.menu_top.text_parent.is_some()) {
        let mouse = system.aim_mouse.as_mut().unwrap();
        let back = mouse.saved.take().unwrap();
        system.cpu.regs = back.regs;
        system.cpu.cpsr = back.cpsr;
        system.cpu.vfp.regs = back.vfp;
        mouse.saved = mouse.menu_top.text_parent.take().map(|saved| *saved)
            .filter(|saved| !matches!(saved.drawing, Drawing::MenuClip(_)));
        return true;
    }
    if place == TOP_TEXT && drawing(system).is_none()
        && system.aim_mouse.as_ref().is_some_and(|mouse| (mouse.menu_top.file_select()
            || mouse.menu_top.active.is_some_and(|(index, _, _, _)| index == 1))
            && !mouse.menu_top.native.is_empty()) {
        system.cpu.regs[15] = TOP_TEXT_DRAWN;
        return true;
    }
    // Solo's heart count is text in layer C, separate from the bag mesh.
    // Both belong to the relocated bag, rather than the native scene.
    if place == TOP_TEXT && drawing(system).is_none()
        && system.cpu.regs[1] == 0xC
        && system.aim_mouse.as_ref().is_some_and(|mouse| !mouse.menu_top.native.is_empty() && mouse.menu_top.active.is_some_and(|(index, ..)| index < ScreenTransform::scripted_len() && ScreenTransform::scripted(index).layout.heart())) {
        system.cpu.regs[15] = TOP_TEXT_DRAWN;
        return true;
    }
    // Only the text pass excludes the old hint location. The background
    // keeps its full-screen viewport and clip throughout its mesh pass.
    let native_text = system.aim_mouse.as_ref().is_some_and(|mouse| mouse.menu_top.active.is_some()
        && !mouse.menu_top.native.is_empty() && mouse.saved.is_none());
    if native_text {
        if place == TOP_TEXT && system.aim_mouse.as_ref().unwrap().menu_top.active.is_some_and(|(index, ..)| !top_layers(index).is_empty()) {
            // The Details card replays after the split artwork, so backgrounds cannot cover its words.
            system.cpu.regs[15] = TOP_TEXT_DRAWN;
            return true;
        }
        if let Some((index, _, _, _)) = system.aim_mouse.as_ref().unwrap().menu_top.active.filter(|&(index, _, _, _)| !details_of(index)) {
            let hint_index = if system.aim_mouse.as_ref().unwrap().menu_top.idol_dialog { 22 } else { index };
            if let Some([_, y, _, height]) = hint_of(hint_index) {
                let width = 400.0 * system.aim_mouse.as_ref().unwrap().widen;
                let rect = if place == TOP_TEXT {
                    [0.0, if y == 0.0 { height } else { 0.0 }, width, 240.0 - height]
                } else { [0.0, 0.0, width, 240.0] };
                system.aim_mouse.as_mut().unwrap().menu_top.clip = Some((Placement {
                    from: rect, to: rect,
                }, true));
                system.aim_mouse.as_mut().unwrap().menu_top.text_parent = Some(Box::new(Saved {
                    regs: system.cpu.regs, cpsr: system.cpu.cpsr, vfp: system.cpu.vfp.regs,
                    drawing: Drawing::MenuClip(place), moved: Vec::new(),
                }));
                set_clip(system, Drawing::Viewport(place), Vec::new());
                return true;
            }
        }
    }
    if let Some(Drawing::MenuTop(step)) = drawing(system) {
        let mouse = system.aim_mouse.as_mut().unwrap();
        if mouse.menu_top.part(step / 8).is_some_and(|part| part.top && (part.model.is_some() || part.backdrop)) {
            if place == TOP_TEXT && mouse.menu_top.active.is_some_and(|(index, ..)|
                mouse.menu_top.part(step / 8).is_some_and(|part|
                    top_layers(index).iter().any(|&(layer, ..)| Some(layer) == part.model))) {
                // Mesh-only layers must not allocate another copy of the details text.
                system.cpu.regs[15] = TOP_TEXT_DRAWN;
                return true;
            }
            mouse.menu_top.drawing_text = place == TOP_TEXT;
            mouse.menu_top.text_parent = mouse.saved.take().map(Box::new);
            set_clip(system, Drawing::Viewport(place), Vec::new());
            return true;
        }
    }
    let native = matches!(drawing(system), None | Some(Drawing::Viewport(TOP_TEXT)));
    // coming back into the text after its viewport is set (r1 is not the
    // layer then), the flag stands
    if drawing(system).is_none() || place != TOP_TEXT {
        if let Some(mouse) = system.aim_mouse.as_mut() {
            mouse.menu_top.cauldron_font = place == TOP_TEXT && native && system.cpu.regs[1] == 0xB
                && mouse.cauldron_on_top_shown(system.frames);
        }
    }
    false
}

pub(super) fn font(system: &mut System) {
    if super::cauldron_text(system) { return; }
    let replay = match drawing(system) {
        Some(Drawing::Viewport(_)) => system.aim_mouse.as_ref().and_then(|mouse| mouse.menu_top.text_parent.as_ref()).map(|saved| saved.drawing),
        replay => replay,
    };
    if let Some(Drawing::MenuTop(step)) = replay {
        if system.aim_mouse.as_ref().is_some_and(|mouse| mouse.menu_top.active.is_some_and(|(index, ..)|
            mouse.menu_top.part(step / 8).is_some_and(|part| part.top
                && part.model.is_some_and(|name| top_layers(index).iter().any(|&(layer, ..)| layer == name))))) {
            // These layers own meshes only; native Details and Hint own glyphs.
            system.cpu.regs[15] += 4;
            return;
        }
        let text_part = system.aim_mouse.as_ref().and_then(|mouse| {
            let (index, ..) = mouse.menu_top.active?;
            mouse.menu_top.part(step / 8).filter(|part| armory_board(index) && !part.top
                && board_text_now(index, part.model, mouse.menu_top.idols_list)).cloned()
        });
        if let Some(part) = text_part {
            let at = system.cpu.regs[0] + 0x24;
            let point = [f32::from_bits(read_word(system, at + 8)), f32::from_bits(read_word(system, at + 12))];
            // Scissoring hides pixels but still allocates every glyph in the
            // shared draw buffer. Emit only this Control's native text; where
            // node layers' boxes overlap (the Idols panel holds its page
            // counter), the smallest box that holds the words owns them.
            let area = |rect: [f32; 4]| rect[2] * rect[3];
            let smaller = system.aim_mouse.as_ref().is_some_and(|mouse| mouse.menu_top.active.is_some_and(|(index, ..)|
                bottom_layer_part(index, part.model) && mouse.menu_top.parts.iter().any(|other| !other.top
                    && other.model != part.model && bottom_layer_part(index, other.model)
                    && board_text_now(index, other.model, mouse.menu_top.idols_list)
                    && inside(point, other.place.from) && area(other.place.from) < area(part.place.from))));
            if !inside(point, part.place.from) || smaller {
                system.cpu.regs[15] += 4;
                return;
            }
        }
    }
    if let Some(Drawing::MenuTop(step)) = drawing(system) {
        if system.aim_mouse.as_ref().is_some_and(|mouse| mouse.menu_top.file_select()
            && mouse.menu_top.part(step / 8).is_some_and(|part| part.top && part.model.is_some())) {
            // The top text pass also draws backdrop meshes; omit only glyphs.
            system.cpu.regs[15] += 4;
            return;
        }
        let part = system.aim_mouse.as_ref().filter(|mouse| mouse.menu_top.file_select())
            .and_then(|mouse| mouse.menu_top.part(step / 8)).filter(|part| !part.top).cloned();
        if let Some(part) = part {
            let at = system.cpu.regs[0] + 0x24;
            let x = f32::from_bits(read_word(system, at + 8));
            let y = f32::from_bits(read_word(system, at + 12));
            // Font anchors already include the placed projection. Undo it
            // only for ownership selection; drawing keeps the projected text.
            let (x, y) = if part.file_book().is_some() {
                let p = part.place;
                let width = 400.0 * system.aim_mouse.as_ref().unwrap().widen;
                (p.from[0] + (x * width / 320.0 - p.to[0]) * p.from[2] / p.to[2],
                    p.from[1] + (y - p.to[1]) * p.from[3] / p.to[3])
            } else { (x, y) };
            // A name's native anchor stays in its book's column while the
            // mesh turns outside the nominal Control box. Select the whole
            // string, including its native shadow, without clipping the book.
            let keep = if let Some(book) = part.file_book() {
                (0.0..320.0).contains(&x) && (0.0..190.0).contains(&y)
                    && (x / (320.0 / 3.0)) as usize == book
            } else { part.model == Some("MenDemoBt0000") && inside([x, y], part.place.from) };
            if !keep {
                system.cpu.regs[15] += 4;
                return;
            }
        }
    }
    let idol_group = match drawing(system) {
        Some(Drawing::MenuTop(step)) => system.aim_mouse.as_ref().and_then(|mouse| mouse.menu_top.part(step / 8)).and_then(|part| part.idol),
        _ => None,
    };
    let counters_width = system.aim_mouse.as_ref().and_then(|mouse| mouse.menu_top.parts.iter()
        .find(|part| part.idol == Some(IdolGroup::Counters)).map(|part| part.place.from[2])).unwrap_or(160.0);
    if let Some(group) = idol_group {
        let at = system.cpu.regs[0] + 0x24;
        let (x, y) = (f32::from_bits(read_word(system, at + 8)), f32::from_bits(read_word(system, at + 12)));
        let keep = match group {
            IdolGroup::Counters => (0.0..counters_width).contains(&x) && (0.0..65.0).contains(&y),
            IdolGroup::Dialog => (65.0..225.0).contains(&y),
            _ => false,
        };
        if !keep || (group == IdolGroup::Counters && system.cpu.regs[15] == 0x00A36F74) {
            system.cpu.regs[15] += 4;
            return;
        }
    }
    let heart = system.aim_mouse.as_ref().is_some_and(|mouse| mouse.menu_top.drawing_text);
    // The game names the hub before its controller's input runs, so its
    // count is held back from the first frame the hub draws.
    let hub = system.aim_mouse.as_ref().is_some_and(|mouse| mouse.menus_on_top) && current_menu(system) == Some(MENUID_HOME);
    let solo = hub || system.aim_mouse.as_ref().is_some_and(|mouse| mouse.menus_on_top
        && (mouse.menu_top.active.map(|(index, ..)| index).or_else(|| mouse.menu_screen.map(|(index, ..)| index)))
            .is_some_and(|index| index < ScreenTransform::scripted_len() && ScreenTransform::scripted(index).layout.heart()));
    if solo {
        let (text, len) = (system.cpu.regs[1], system.cpu.regs[2]);
        let number = if (1..=10).contains(&len) && system.memory.is_readable(text, len * 2) {
            let mut bytes = vec![0; len as usize * 2];
            system.memory.read_bytes(text, &mut bytes);
            bytes.as_chunks::<2>().0.iter().all(|word| (b'0' as u16..=b'9' as u16).contains(&u16::from_le_bytes(*word)))
        } else { false };
        // The counter belongs only to its relocated bag, even when a pan
        // moves the native number into the hint's source rectangle.
        if number != heart {
            system.cpu.regs[15] += 4;
            return;
        }
    }
    if !heart { return; }
    // The native number effect draws an offset copy before its main text.
    // At the compact scale it looks like a second count; keep only the main pass.
    if system.cpu.regs[15] == 0x00A36F74 {
        system.cpu.regs[15] = 0x00A36F78;
        return;
    }
    let at = system.cpu.regs[0] + 0x24;
    let mut old = [0; 16];
    system.memory.read_bytes(at, &mut old);
    let values: Vec<f32> = old.as_chunks::<4>().0.iter().map(|word| f32::from_le_bytes(*word)).collect();
    if values.iter().any(|value| !value.is_finite()) { return; }
    let values = [values[0] * 0.5, values[1] * 0.5, values[2], values[3]];
    let bytes: Vec<u8> = values.into_iter().flat_map(f32::to_le_bytes).collect();
    system.memory.write_bytes(at, &bytes);
    system.aim_mouse.as_mut().unwrap().menu_top.font_back = Some((at, old));
}

/// the game's menu slot: a holder whose +0xC is the menu, whose +0x72C is
/// its MENUID (analysis/FINDINGS.md, "The game's menu identity").
const MENU_SLOT: u32 = 0x00E6F98C;
const MENU_HOLDER: u32 = 0x00CFC5C0;
const MENU_BODY: u32 = 0x00CFC314;
const MENUID_HOME: u8 = 3;

/// each scripted Screen and the MENUIDs it is shown under, measured on the
/// trace states. Dialogs show under their parent's MENUID, and both Idol
/// Tosses under 44, so those take the Screen the input last confirmed.
const SCREEN_MENUS: [(usize, &[u8]); 27] = [
    (0, &[2]), (1, &[1]), (2, &[4]), (3, &[17]), (4, &[22]), (5, &[19]), (6, &[25]),
    (7, &[23]), (8, &[21]), (9, &[39]), (10, &[45]), (11, &[]), (12, &[3]), (13, &[11]),
    (14, &[14]), (15, &[12]), (16, &[37]), (17, &[26]), (18, &[18]), (19, &[43]),
    (20, &[42]), (21, &[10]), (22, &[44]), (23, &[13]), (24, &[16]), (25, &[44]), (26, &[46]),
];

/// the MENUIDs a scripted Screen is shown under, empty for any.
fn menus_of(index: usize) -> &'static [u8] {
    SCREEN_MENUS.iter().find(|&&(at, _)| at == index).map_or(&[], |&(_, ids)| ids)
}

/// the one scripted Screen a MENUID shows, none when it shows several or
/// none (the Cauldron, Vault pages without a layout).
fn screen_of(id: u8) -> Option<usize> {
    let mut screens = SCREEN_MENUS.iter().filter(|&&(_, ids)| ids.contains(&id)).map(|&(index, _)| index);
    let first = screens.next()?;
    screens.next().is_none().then_some(first)
}

/// the MENUID the game has current, none outside a menu.
pub fn current_menu(system: &mut System) -> Option<u8> {
    let holder = read_word(system, MENU_SLOT);
    if holder == 0 || !system.memory.is_readable(holder, 0x10) || read_word(system, holder) != MENU_HOLDER { return None; }
    let body = read_word(system, holder + 0xC);
    if body == 0 || !system.memory.is_readable(body, 0x730) || read_word(system, body) != MENU_BODY { return None; }
    let mut id = [0u8];
    system.memory.read_bytes(body + 0x72C, &mut id);
    Some(id[0])
}

pub(super) fn font_done(system: &mut System) {
    if let Some((at, old)) = system.aim_mouse.as_mut().and_then(|mouse| mouse.menu_top.font_back.take()) {
        system.memory.write_bytes(at, &old);
    }
}

pub(super) fn bottom_translucent(system: &mut System) {
    let Some(Drawing::MenuTop(step)) = drawing(system) else { return };
    if system.aim_mouse.as_ref().is_some_and(|mouse|
        mouse.menu_top.active.is_some_and(|(index, _, _, _)| matches!(index, 22 | 25) || mouse.menu_top.idol_dialog)
            && !mouse.menu_top.part(step / 8).and_then(|part| part.idol).is_some_and(|group| group == IdolGroup::Launcher || group.scenery())) {
        // Idol Toss's translucent pass shades the bowl and is drawn with
        // the launcher and the scenery, whose replays hide every other
        // mesh. The counters, Back and the dialog are drawn without it.
        system.cpu.regs[15] = BOTTOM_TRANSLUCENT + 4;
    }
}

pub(super) fn bottom_text(system: &mut System) {
    if system.aim_mouse.as_ref().is_some_and(|mouse| mouse.menu_top.file_select()) {
        if let Some(Drawing::MenuTop(step)) = drawing(system) {
            if system.aim_mouse.as_ref().unwrap().menu_top.part(step / 8).is_some_and(|part| !part.top && part.model.is_none()) {
                system.cpu.regs[15] = BOTTOM_TEXT + 4;
            }
        }
        return;
    }
    let step = match drawing(system) { Some(Drawing::MenuTop(step)) => Some(step), _ => None };
    let Some(mouse) = system.aim_mouse.as_ref() else { return };
    if let Some((index, ..)) = mouse.menu_top.active.filter(|&(index, ..)| drawn_cut(index)) {
        // Armory text is one dense pass; replay it only where a Control shows text.
        if armory_board(index) && step.and_then(|step| mouse.menu_top.part(step / 8))
            .is_some_and(|part| !part.top && part.model.is_some() && !board_text_now(index, part.model, mouse.menu_top.idols_list)) {
            system.cpu.regs[15] = BOTTOM_TEXT + 4;
        }
        return;
    }
    if let Some(Drawing::MenuTop(step)) = drawing(system) {
        let menu_top = &system.aim_mouse.as_ref().unwrap().menu_top;
        let row = menu_top.part_index(step / 8).and_then(|index| menu_top.row_chapter(index)).is_some();
        if row || menu_top.part(step / 8).is_some_and(|part| !part.top && (part.model.is_some() || part.idol.is_some_and(|group| !matches!(group, IdolGroup::Counters | IdolGroup::Dialog)))) {
            // Hub labels and chapter labels are textures in their meshes.
            // Unrelated text in the other groups must not be replayed with
            // them, and an uncut Row would show the hint enlarged.
            system.cpu.regs[15] = BOTTOM_TEXT + 4;
        }
    }
}

fn target(system: &mut System, eye: u32, step: usize, mut moved: Vec<(u32, Vec<u8>)>) {
    let manager = read_word(system, VSYNC_MANAGER);
    change(system, &mut moved, manager + 0x210, &eye.to_le_bytes());
    call_here(system, 0x00916588, manager, eye, Drawing::MenuTop(step), moved);
}

/// Circle shadows draw in every replay of a split top screen, at that
/// replay's Placement. Only the layer owning `CIRCLE_SHADOW` keeps them;
/// any other replay, and the clipped native pass, returns at once.
pub(super) fn circle_shadow(system: &mut System) {
    let Some(mouse) = system.aim_mouse.as_ref() else { return };
    let Some((index, ..)) = mouse.menu_top.active else { return };
    let Some(&(owner, ..)) = top_layers(index).iter().find(|&&(_, model, _)| model == super::screen_transform::CIRCLE_SHADOW) else { return };
    let skip = match drawing(system) {
        Some(Drawing::MenuTop(step)) => mouse.menu_top.part(step / 8).is_none_or(|part| part.model != Some(owner)),
        Some(Drawing::MenuClip(_)) => true,
        _ => false,
    };
    if skip { system.cpu.regs[15] = system.cpu.regs[14]; }
}

pub(super) fn draw(system: &mut System) -> bool {
    let mut top = system.cpu.regs[4];
    let back = system.aim_mouse.as_mut().and_then(|mouse| mouse.saved.take_if(|saved| matches!(saved.drawing, Drawing::MenuTop(_))));
    let next = if let Some(back) = back {
        system.cpu.regs = back.regs;
        system.cpu.cpsr = back.cpsr;
        system.cpu.vfp.regs = back.vfp;
        top = back.regs[4];
        let Drawing::MenuTop(step) = back.drawing else { unreachable!() };
        if step == usize::MAX {
            system.aim_mouse.as_mut().unwrap().menu_top.clip = None;
            set_clip(system, Drawing::MenuTop(usize::MAX - 1), Vec::new());
            return true;
        }
        if step == usize::MAX - 1 { return true; }
        let part = system.aim_mouse.as_ref().unwrap().menu_top.part(step / 8).cloned();
        if let Some(part) = part {
            let scene = read_word(system, top + 0x40);
            if step % 8 == 0 {
                if part.top { call_here(system, TOP_SCENE_SETUP, top, scene, Drawing::MenuTop(step + 1), back.moved); }
                else { target(system, 0, step + 1, back.moved); }
                return true;
            }
            if step % 8 == 1 {
                let bottom = system.aim_mouse.as_ref().unwrap().bottom_scene.unwrap();
                call_here(system, if part.top { TOP_SCENE_3D_DRAW } else { BOTTOM_SCENE_DRAW }, if part.top { top } else { bottom }, scene, Drawing::MenuTop(step + 1), back.moved);
                return true;
            }
        }
        put_back(system, &back.moved);
        step / 8 + 1
    } else {
        if system.aim_mouse.as_ref().is_none_or(|mouse| mouse.menu_top.active.is_none()) { return false; }
        let moved = system.aim_mouse.as_mut().map(|mouse| std::mem::take(&mut mouse.menu_top.native)).unwrap_or_default();
        put_back(system, &moved);
        if scene(system, top).is_none() { return false; }
        0
    };
    let Some((index, object, bottom, _)) = scene(system, top) else { return true };
    let part = system.aim_mouse.as_ref().unwrap().menu_top.part(next).cloned();
    let Some(part) = part else {
        let mouse = system.aim_mouse.as_mut().unwrap();
        mouse.menu_top.duration = system.frames.saturating_sub(mouse.menu_top.started);
        mouse.menu_top.active = None;
        mouse.menu_top.drawn = Some((system.frames, index, object, mouse.widen));
        let manager = read_word(system, VSYNC_MANAGER);
        let view = [0x44, 0x48, 0x4C, 0x50].map(|at| read_word(system, top + at));
        if manager != 0 {
            call_here(system, SET_VIEWPORT, manager, view[0], Drawing::MenuTop(usize::MAX), Vec::new());
            system.cpu.regs[2] = view[1];
            system.cpu.regs[3] = view[2];
            let sp = system.cpu.regs[13] - 8;
            system.cpu.regs[13] = sp;
            system.memory.write_bytes(sp, &view[3].to_le_bytes());
        }
        return true;
    };
    let drawing = if part.top { top } else { bottom };
    let scene = read_word(system, top + 0x40);
    let renderer = read_word(system, scene + 4);
    let mut moved = Vec::new();
    // Both drawings borrow the same renderer and cameras. Restore every
    // changed value before another group, eye, or native bottom pass.
    for (at, len) in [(scene + 0xF4, 8), (renderer + 0x58, 4)] {
        let mut bytes = vec![0; len];
        system.memory.read_bytes(at, &mut bytes);
        moved.push((at, bytes));
    }
    let row = {
        let menu_top = &system.aim_mouse.as_ref().unwrap().menu_top;
        menu_top.part_index(next).and_then(|index| menu_top.row_chapter(index)).map(|chapter| RowDraw {
            chapter,
            center: [part.place.from[0] + part.place.from[2] * 0.5, part.place.from[1] + part.place.from[3] * 0.5],
            outlined: menu_top.chapter_outlined == Some(chapter),
        })
    };
    let node_layer = part.top && part.model.is_some_and(|name| top_layers(index).iter().any(|&(layer, ..)| layer == name));
    // The Details card replayed after split layers carries the top words;
    // the native viewport mapping squeezes their advance, so it takes the
    // guard band like other top parts.
    if !node_layer && part.top && (part.model.is_some() || part.backdrop) && !system.aim_mouse.as_ref().unwrap().menu_top.file_select() {
        // A centered guard band extends the camera's clip planes without
        // changing its screen transform or the font renderer's alignment.
        let p = part.place;
        let scale = [p.to[2] / p.from[2], p.to[3] / p.from[3]];
        let from = [-200.0, -120.0, 800.0, 480.0];
        let to = [p.to[0] + (from[0] - p.from[0]) * scale[0], p.to[1] + (from[1] - p.from[1]) * scale[1], from[2] * scale[0], from[3] * scale[1]];
        place_scene(system, top, drawing, Placement { from, to }, true, true, &mut moved);
    } else if !node_layer && (part.file_book().is_some() || row.is_some() || part.idol.is_some_and(IdolGroup::uncut) || part.model.is_some() && !part_cut(index, part.model)
        && !system.aim_mouse.as_ref().unwrap().menu_top.file_select()) {
        let widen = system.aim_mouse.as_ref().unwrap().widen;
        let canvas = [400.0 * widen, 240.0];
        let scale = [part.place.to[2] / part.place.from[2], part.place.to[3] / part.place.from[3]];
        let source = [part.place.from[0] - part.place.to[0] / scale[0],
            part.place.from[1] - part.place.to[1] / scale[1], canvas[0] / scale[0], canvas[1] / scale[1]];
        // Put the transform in the projection so the viewport never cuts
        // an animated mesh at its original screen or nominal button box.
        place_scene(system, top, drawing, Placement { from: source, to: [0.0, 0.0, canvas[0], canvas[1]] }, part.top, true, &mut moved);
    } else {
        // Node groups keep the native viewport mapping; enlarging it for a
        // guard band also enlarges models whose camera setup resets the lens.
        place_scene(system, top, drawing, part.place, part.top, false, &mut moved);
    }
    if system.aim_mouse.as_ref().unwrap().menu_top.file_select() {
        file_models(system, drawing, Some(&part), &mut moved);
    } else if part.chapter {
        chapter_models(system, drawing, part.top, true, row, &mut moved);
    } else if part.top {
        // Keep the layers present so their text renderers still run, but
        // hide unrelated meshes instead of replaying scenery in each item.
        let layers = read_word(system, scene + SCENE_LAYERS);
        for layer in 0..16 {
            let list = read_word(system, layers + layer * LAYER_SIZE + 4);
            let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
            if list == 0 || end < start || end - start > 0x1000 { continue; }
            for at in (start..end).step_by(4) {
                let model = read_word(system, at);
                let name = model_name(system, model);
                let selected = part.model.is_some_and(|layer| layer == name);
                show_parts(system, &mut moved, model, |node| if part.backdrop {
                    top_layer(top_layers(index), &name, node).is_none()
                } else if part.model.is_some_and(|layer| top_layers(index).iter().any(|&(name, ..)| name == layer)) {
                    top_layer(top_layers(index), &name, node) == part.model
                } else { selected && (part.model != Some("MenCmnUpHeart0000") || node != "capbase") });
            }
        }
    } else if let Some(group) = part.idol {
        // Scene callbacks also retain their own model references. Hide
        // unselected meshes, rather than just shortening the layer lists.
        let layers = read_word(system, scene + SCENE_LAYERS);
        for layer in BOTTOM_LAYERS {
            let list = read_word(system, layers + layer * LAYER_SIZE + 4);
            if list == 0 { continue; }
            let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
            if end < start || end - start > 0x1000 { continue; }
            for at in (start..end).step_by(4) {
                let model = read_word(system, at);
                let name = model_name(system, model);
                let egg = (name == "MenColVisionBt0002").then(|| idol_egg_group(system, model));
                if group != IdolGroup::Launcher && !group.scenery() && group.owns(&name) { foreground_materials(system, model, &mut moved); }
                show_nodes(system, &mut moved, model, |node| egg.is_none_or(|owner| owner == group) && group.shows(&name, node));
            }
        }
    } else if bottom_layer_part(index, part.model) {
        // A bottom node layer shows only its own nodes of its model. The
        // other models leave the replay, which would otherwise exhaust the
        // game's drawing buffer once per layer.
        let owner = bottom_layers(index).iter().find(|&&(layer, ..)| Some(layer) == part.model).map(|&(_, owner, _)| owner);
        filter_models(system, drawing, false, true, &mut moved, |name| Some(name) == owner);
        let layers = read_word(system, scene + SCENE_LAYERS);
        for layer in BOTTOM_LAYERS {
            let list = read_word(system, layers + layer * LAYER_SIZE + 4);
            if list == 0 { continue; }
            let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
            if end < start || end - start > 0x1000 { continue; }
            for at in (start..end).step_by(4) {
                let model = read_word(system, at);
                let name = model_name(system, model);
                if Some(name.as_str()) != owner { continue; }
                show_nodes(system, &mut moved, model, |node| top_layer(bottom_layers(index), &name, node) == part.model);
            }
        }
    } else {
        let slider = matches!(part.model, Some("MenOptionSoundBt0002" | "MenOptionSoundBt0003" | "MenOptionSoundBt0004"));
        filter_models(system, drawing, part.top, !part.top, &mut moved, |name|
            part.model.is_none_or(|model| name == model || board_companions(model).contains(&name))
                || (slider && matches!(name, "MenOptionSoundBt0001" | "MenOptionSoundBt0006")));
    }
    if !part.top { change(system, &mut moved, bottom + 0x60, &[0; 4]); }
    system.aim_mouse.as_mut().unwrap().menu_top.clip = Some((part.place, part.top));
    set_clip(system, Drawing::MenuTop(next * 8), moved);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn details_text_draws_after_split_artwork_instead_of_under_its_background() {
        let mut system = System::new(crate::Config::default());
        let bottom = 0x14000000;
        let scene = bottom + 0x100;
        system.memory.map(bottom, 0x20000000, 0x1000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        system.memory.write_bytes(bottom + 0x40, &scene.to_le_bytes());
        system.memory.write_bytes(scene + SCENE_LAYERS, &(bottom + 0x200).to_le_bytes());
        for (screen, details) in [(14, 4), (18, 6)] {
            let parts = layout(screen, 400.0).1;
            assert_eq!(draw_order(&mut system, bottom, &parts, true, &[]).last(), Some(&details));
            assert!(!draw_order(&mut system, bottom, &parts, false, &[]).contains(&details));

            let mut mouse = shown(screen);
            mouse.menu_top.active = Some((screen, 0, 0, false));
            mouse.menu_top.native = vec![(0, vec![])];
            system.aim_mouse = Some(mouse);
            system.cpu.regs[15] = TOP_TEXT;
            assert!(text(&mut system, TOP_TEXT));
            assert_eq!(system.cpu.regs[15], TOP_TEXT_DRAWN);
        }
    }

    #[test]
    fn artwork_layers_skip_details_text_before_it_allocates_draw_commands() {
        let mut system = System::new(crate::Config::default());
        for (screen, first) in [(14, 5), (18, 7)] {
            for part in first..first + top_layers(screen).len() {
                let mut mouse = shown(screen);
                mouse.menu_top.active = Some((screen, 0, 0, false));
                mouse.menu_top.draw_order = vec![part];
                mouse.saved = Some(Saved { regs: [0; 16], cpsr: system.cpu.cpsr, vfp: [0; 32], drawing: Drawing::MenuTop(0), moved: Vec::new() });
                system.aim_mouse = Some(mouse);
                system.cpu.regs[15] = TOP_TEXT;
                assert!(text(&mut system, TOP_TEXT));
                assert_eq!(system.cpu.regs[15], TOP_TEXT_DRAWN);
                assert!(system.aim_mouse.as_ref().unwrap().menu_top.text_parent.is_none());
            }
        }
    }

    #[test]
    fn gear_node_groups_hide_independently_and_restore_native_visibility() {
        let mut system = System::new(crate::Config::default());
        system.memory.map(0x14000000, 0x20000000, 0x4000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        let model = 0x14000000;
        let cmdl = model + 0x400;
        let dictionary = cmdl + 0x200;
        let nodes = model + 0x1000;
        let names = ["NN_U0000", "NN_U0001", "NN_U0002", "NN_U0027", "NN_U0010"];
        for (at, value) in [(model + 8, cmdl), (cmdl + 0xD0, dictionary - cmdl - 0xD0),
            (dictionary + 8, names.len() as u32), (model + MODEL_NODES, nodes),
            (model + MODEL_NODES + 4, nodes + names.len() as u32 * 8)] {
            system.memory.write_bytes(at, &value.to_le_bytes());
        }
        for (i, name) in names.iter().enumerate() {
            let entry = dictionary + 0x24 + i as u32 * 0x10;
            let text = model + 0x2000 + i as u32 * 0x40;
            system.memory.write_bytes(entry, &(text - entry).to_le_bytes());
            system.memory.write_bytes(text, name.as_bytes());
            system.memory.write_bytes(nodes + i as u32 * 8 + 4, &[1]);
        }
        let layers = top_layers(14);
        for selected in [None, Some("Paper background"), Some("Silhouette"), Some("Crest"), Some("Ornaments")] {
            let mut moved = Vec::new();
            show_nodes(&mut system, &mut moved, model, |node| top_layer(layers, "MenEquipUp0000", node) == selected);
            for (i, owner) in ["Paper background", "Paper background", "Crest", "Silhouette", "Ornaments"].iter().enumerate() {
                assert_eq!(read_word(&mut system, nodes + i as u32 * 8 + 4), u32::from(selected == Some(*owner)));
            }
            put_back(&mut system, &moved);
            for i in 0..names.len() {
                assert_eq!(read_word(&mut system, nodes + i as u32 * 8 + 4), 1);
            }
        }
        assert_eq!(top_layer(layers, "MenEquipUp0001", "NN_U0000"), Some("Page title"));
        assert_eq!(top_layer(layers, "MenEquipUp0001", "NN_U00022"), Some("Weapon"));
        assert_eq!(top_layer(layers, "MenEquipUp0001", "powers"), None, "unowned nodes stay in the native Details card");
        for part in 5..11 {
            for viewport in [false, true] {
                let mut mouse = shown(14);
                mouse.menu_top.active = Some((14, 0x14050000, 0, false));
                mouse.menu_top.draw_order = vec![part];
                let saved = || Saved { regs: [0; 16], cpsr: system.cpu.cpsr, vfp: [0; 32], drawing: Drawing::MenuTop(0), moved: Vec::new() };
                mouse.saved = Some(saved());
                if viewport {
                    mouse.menu_top.text_parent = Some(Box::new(saved()));
                    mouse.saved.as_mut().unwrap().drawing = Drawing::Viewport(TOP_TEXT);
                }
                system.aim_mouse = Some(mouse);
                system.cpu.regs[15] = 0x00A36FA4;
                font(&mut system);
                assert_eq!(system.cpu.regs[15], 0x00A36FA8, "mesh layers must not duplicate Hint glyphs");
            }
        }
    }

    /// an edit made at one width keeps its edges and stays on screen at
    /// another: the Vault's Back stays left, AR flush right, and a box
    /// placed at any width is the same box when that width returns.
    #[test]
    fn editor_overrides_follow_the_window_width() {
        let back = [0.0, 216.68, 26.5, 23.32];
        let ar = [368.2, 215.62, 31.8, 24.38];
        for width in [240.0, 320.0, 400.0, 240.0 * 16.0 / 9.0, 560.0, 800.0] {
            let [x, _, w, _] = seat_override(back, width);
            assert_eq!(x, 0.0);
            let [x, y, w2, h] = seat_override(ar, width);
            assert!((x + w2 - width).abs() < 0.01 && (y + h - 240.0).abs() < 0.01, "AR flush at {width}");
            assert!(w <= 26.5 && w2 <= 31.8);
            for to in [[25.3, 150.7, 90.1, 58.3], [-6.3, 122.4, 145.6, 55.8], [-1.1, -9.8, 400.5, 248.6]] {
                let placed = seat_override(to, width);
                let back = seat_override(unseat_override(placed, width), width);
                assert!(placed.iter().zip(back).all(|(a, b)| (a - b).abs() < 0.01), "{to:?} at {width}");
            }
        }
    }

    #[test]
    fn layouts_preserve_shapes_and_fit_all_window_sizes() {
        for index in 0..ScreenTransform::scripted_len() + 9 {
            for width in [240.0, 320.0, 400.0, 240.0 * 16.0 / 9.0, 560.0, 800.0] {
                let (native, parts) = layout(index, width);
                if !details_of(index) {
                    assert_eq!(native.from, [0.0, 0.0, 400.0, 240.0], "keep the whole top scene");
                    assert_eq!(native.to, [0.0, 0.0, width, 240.0], "background fills the viewport");
                }
                for part in std::iter::once(native).chain(parts.into_iter().map(|part| part.place)) {
                    assert!(part.to.iter().all(|value| value.is_finite()));
                    assert!(part.to[0] >= -0.001 && part.to[1] >= -0.001);
                    assert!(part.to[0] + part.to[2] <= width + 0.001);
                    assert!(part.to[1] + part.to[3] <= 240.001);
                    // Scenery may fill its area, controls always keep their shape.
                    if part.from != native.from || part.to != native.to {
                        assert!((part.to[2] / part.from[2] - part.to[3] / part.from[3]).abs() < 0.001);
                    }
                    let point = [part.from[0] + part.from[2] * 0.37, part.from[1] + part.from[3] * 0.61];
                    let top = mapped(point, part.from, part.to);
                    let back = mapped([top.0, top.1], part.to, part.from);
                    assert!((point[0] - back.0).abs() < 0.001 && (point[1] - back.1).abs() < 0.001);
                }
            }
        }
    }

    #[test]
    fn chapter_details_stay_above_controls_and_do_not_take_mouse_input() {
        for width in [240.0, 400.0, 240.0 * 16.0 / 9.0, 800.0] {
            let (native, parts) = layout(1, width);
            assert_eq!(native.to, [0.0, 0.0, width, 240.0]);
            let details = parts.iter().find(|part| part.top).unwrap().place;
            let controls = parts.iter().find(|part| !part.top).unwrap().place;
            assert!(details.to[1] + details.to[3] < controls.to[1]);
            let mut mouse = shown(1);
            mouse.widen = width / 400.0;
            mouse.menu_top.drawn.as_mut().unwrap().3 = mouse.widen;
            mouse.menu_top.parts = parts;
            assert_eq!(mouse.menus_top_touch(11, width * 0.5 / mouse.widen, 40.0, false), None);
            // Back and the Hint strip; the thumbnails are Rows, built per frame
            for point in [(35.0, 215.0), (240.0, 200.0)] {
                let top = mouse.menus_top_point(11, point.0, point.1, None).unwrap();
                assert_eq!(mouse.menus_top_touch(11, top.0, top.1, false), Some(point));
            }
        }
    }

    #[test]
    fn chapter_hover_survives_the_list_scrolling_until_a_click() {
        let mut mouse = shown(1);
        mouse.mouse_mode = true;
        mouse.menu_top.pointer = Some([200.0, 150.0]);
        mouse.pointer = Some((80.0, 40.0));
        mouse.menu_looked = Some(((80.0, 40.0), 10));
        refresh_hover(&mut mouse, 11);
        assert_eq!(mouse.menu_looked, Some(((80.0, 40.0), 10)));
    }

    #[test]
    fn chapter_rows_fill_one_row_above_back_and_the_wheel_stops_at_both_ends() {
        let mut fits = Vec::new();
        for width in [240.0, 400.0, 240.0 * 16.0 / 9.0, 560.0] {
            let (slots, fit) = chapter_slots(width, 26, 0);
            assert_eq!(slots.len(), fit);
            for (_, to) in &slots {
                assert!(to[0] >= 0.0 && to[0] + to[2] <= width && to[1] + to[3] <= CHAPTER_ROW_BOTTOM);
                assert_eq!(to[1], slots[0].1[1], "one row");
            }
            fits.push(fit);
            // a short List starts at the left whatever the scroll
            assert_eq!(chapter_slots(width, 1, 9).0, vec![(0, slots[0].1)]);
            // the last chapter ends the row
            assert_eq!(chapter_slots(width, 26, 99).0.last().unwrap().0, 25);
        }
        assert!(fits.windows(2).all(|pair| pair[0] <= pair[1]) && fits[0] < fits[3], "wider windows show more chapters");

        let mut mouse = shown(1);
        mouse.menu_top.chapter_span = (26, 5);
        mouse.scroll_chapters(-3.0);
        assert_eq!(mouse.menu_top.chapter_scroll, 0);
        mouse.scroll_chapters(2.5);
        mouse.scroll_chapters(0.5);
        assert_eq!(mouse.menu_top.chapter_scroll, 3, "fractions add up");
        mouse.scroll_chapters(40.0);
        assert_eq!(mouse.menu_top.chapter_scroll, 21);
    }

    #[test]
    fn idol_reveal_hides_launcher_and_fountain_until_tossing_returns() {
        let mut system = System::new(crate::Config::default());
        let object = 0x14000000;
        system.memory.map(object, 0x20000000, 0x1000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        for screen in [22, 25] {
            let mut mouse = shown(screen);
            for phase in [0_u32, 1, 2, 3, 4, 5, 0] {
                system.memory.write_bytes(object + 0x430, &phase.to_le_bytes());
                mouse.menu_top.idol_revealing = idol_revealing(&mut system, object);
                for part in &mouse.menu_top.parts {
                    let hidden = (2..=4).contains(&phase)
                        && matches!(part.idol, Some(IdolGroup::Launcher | IdolGroup::Fountain));
                    assert_eq!(mouse.menu_top.enabled(part), !hidden);
                    assert!(part.visible, "native phases must preserve editor visibility");
                }
                let bowl = mouse.menu_top.parts[2].place.map_point([160.0, 100.0]);
                mouse.clear_menus_top_pointer();
                assert_eq!(mouse.menus_top_touch(11, bowl[0], bowl[1], false).is_some(), !(2..=4).contains(&phase));
            }
        }
    }

    #[test]
    fn idol_eggs_draw_with_the_launcher_after_leaving_the_queue() {
        let mut system = System::new(crate::Config::default());
        let model = 0x14000000;
        let skeleton = model + 0x400;
        let world = model + 0x800;
        system.memory.map(model, 0x20000000, 0x1000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        system.memory.write_bytes(model + MODEL_SKELETON, &skeleton.to_le_bytes());
        system.memory.write_bytes(skeleton + SKELETON_WORLD, &world.to_le_bytes());
        for (x, expected) in [(13.5_f32, IdolGroup::Eggs), (0.79, IdolGroup::Launcher)] {
            system.memory.write_bytes(world + 0xC, &x.to_le_bytes());
            assert_eq!(idol_egg_group(&mut system, model), expected);
            assert!(expected.shows("MenColVisionBt0002", "egg3"));
        }
    }

    #[test]
    fn idol_egg_drag_reaches_the_bowl_through_edited_placements() {
        for screen in [22, 25] {
            let mut mouse = shown(screen);
            mouse.set_editor_layer(2, [140.0, 100.0, 168.0, 77.0]);
            mouse.set_editor_layer(6, [340.0, 0.0, 58.0, 240.0]);
            let egg = mouse.menu_top.parts[6].place;
            let bowl = mouse.menu_top.parts[2].place;
            let start = egg.map_point([290.0, 20.0]);
            let end = bowl.map_point([160.0, 100.0]);
            assert_eq!(mouse.menus_top_touch(11, start[0], start[1], true), Some((290.0, 20.0)));
            assert_eq!(mouse.menus_top_touch(11, end[0], end[1], true), Some((160.0, 100.0)));
            mouse.menus_top_touch(11, end[0], end[1], false);
            assert!(!mouse.menus_top_dragging());
        }
    }

    #[test]
    fn idol_mesh_groups_do_not_clip_at_their_placement_boxes() {
        let mut system = System::new(crate::Config::default());
        let at = 0x14000000;
        system.memory.map(at, 0x20000000, 0x1000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        for screen in [22, 25] {
            for index in [1, 2, 5, 6] {
                let mut mouse = shown(screen);
                let part = mouse.menu_top.parts[index].clone();
                mouse.menu_top.draw_order = vec![index];
                mouse.menu_top.clip = Some((part.place, part.top));
                mouse.saved = Some(Saved { regs: [0; 16], cpsr: system.cpu.cpsr, vfp: [0; 32], drawing: Drawing::MenuTop(0), moved: Vec::new() });
                system.aim_mouse = Some(mouse);
                system.cpu.regs[2] = at;
                scissor(&mut system);
                assert_eq!((0..4).map(|i| read_word(&mut system, at + i * 4)).collect::<Vec<_>>(), vec![0, 0, 240, 400]);
                scissor_done(&mut system);
            }
        }
    }

    #[test]
    fn idol_corner_controls_and_launcher_share_their_input_transforms() {
        for index in [22, 25] {
            for width in [240.0, 400.0, 240.0 * 16.0 / 9.0, 560.0] {
                let mut mouse = shown(index);
                mouse.widen = width / 400.0;
                mouse.menu_top.parts = layout(index, width).1;
                mouse.menu_top.drawn.as_mut().unwrap().3 = mouse.widen;
                let parts = &mouse.menu_top.parts;
                let counters = parts.iter().find(|part| part.idol == Some(IdolGroup::Counters)).unwrap().place;
                let back = parts.iter().find(|part| part.idol == Some(IdolGroup::Back)).unwrap().place;
                let launcher = parts.iter().find(|part| part.idol == Some(IdolGroup::Launcher)).unwrap().place;
                assert_eq!(&counters.to[..2], &[0.0, 14.0]);
                assert_eq!(back.to[0], 8.0);
                assert_eq!(back.to[1] + back.to[3], 232.0);
                let fountain = parts.iter().find(|part| part.idol == Some(IdolGroup::Fountain)).unwrap().place;
                let eggs = parts.iter().find(|part| part.idol == Some(IdolGroup::Eggs)).unwrap().place;
                assert_eq!(eggs.to, [width - 58.0, 0.0, 58.0, 240.0], "the queue keeps its size down the right edge");
                let scale = launcher.to[2] / launcher.from[2];
                assert!((launcher.map_point([eggs.from[0], 0.0])[0] - eggs.to[0]).abs() < 0.001, "the bowl is as far from the queue as on the bottom screen");
                assert!((fountain.to[2] / fountain.from[2] - scale).abs() < 0.001);
                for axis in 0..2 {
                    assert!((launcher.map_point([fountain.from[0], fountain.from[1]])[axis] - fountain.to[axis]).abs() < 0.001, "the fountain stands under the bowl");
                }
                assert_eq!(parts.iter().position(|part| part.backdrop), Some(4), "the scenery follows the layers saved before it");
                assert_eq!(mouse.menus_top_touch(11, (eggs.to[0] + 29.0) / mouse.widen, 120.0, false), Some((291.0, 120.0)));
                for native in [(30.0, 215.0), (160.0, 100.0)] {
                    let top = mouse.menus_top_point(11, native.0, native.1, None).unwrap();
                    assert_eq!(mouse.menus_top_touch(11, top.0, top.1, false), Some(native));
                }
                let coin = mapped([30.0, 46.0], counters.from, counters.to);
                assert_eq!(mouse.menus_top_touch(11, coin.0 / mouse.widen, coin.1, true), Some((30.0, 46.0)));
                assert!(inside([30.0, 64.0], counters.from), "keep the coin button's lower border");
            }
        }
    }

    #[test]
    fn coin_dialog_keeps_background_and_blocks_parent_controls() {
        for width in [240.0, 400.0, 560.0] {
            let (native, parts) = layout_with_dialog(11, width, true);
            assert_eq!(native.to, [0.0, 0.0, width, 240.0]);
            assert_eq!(parts.iter().filter(|part| part.top && part.model.is_none() && !part.backdrop).count(), 1);
            let coin = parts.iter().find(|part| part.idol == Some(IdolGroup::Counters)).unwrap().place;
            let back = parts.iter().find(|part| part.idol == Some(IdolGroup::Back)).unwrap().place;
            let mut mouse = shown(11);
            mouse.widen = width / 400.0;
            mouse.menu_top.drawn.as_mut().unwrap().3 = mouse.widen;
            mouse.menu_top.parts = parts;
            mouse.menu_top.idol_dialog = true;
            for point in [mapped([30.0, 46.0], coin.from, coin.to), mapped([30.0, 215.0], back.from, back.to)] {
                assert_eq!(mouse.menus_top_touch(11, point.0 / mouse.widen, point.1, false), None);
            }
            for native in [(90.0, 175.0), (230.0, 175.0)] {
                let point = mouse.menus_top_point(11, native.0, native.1, None).unwrap();
                assert_eq!(mouse.menus_top_touch(11, point.0, point.1, false), Some(native));
            }
        }
    }

    #[test]
    fn coin_labels_omit_offset_copy_and_unrelated_dialog_text() {
        let mut system = System::new(crate::Config::default());
        system.memory.map(0x14000000, 0x20000000, 0x1000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        let mut mouse = shown(22);
        mouse.saved = Some(Saved { regs: [0; 16], cpsr: system.cpu.cpsr, vfp: [0; 32], drawing: Drawing::MenuTop(2), moved: Vec::new() });
        system.aim_mouse = Some(mouse);
        system.cpu.regs[0] = 0x14000000;
        for (pc, x, y, expected) in [(0xA36F74, 7.0_f32, 15.0_f32, 0xA36F78),
            (0xA36FA4, 6.0, 15.0, 0xA36FA4), (0xA36FA4, 160.0, 93.0, 0xA36FA8)] {
            system.cpu.regs[15] = pc;
            system.memory.write_bytes(0x1400002C, &x.to_le_bytes());
            system.memory.write_bytes(0x14000030, &y.to_le_bytes());
            font(&mut system);
            assert_eq!(system.cpu.regs[15], expected);
        }
    }

    #[test]
    fn smaller_hub_controls_keep_their_original_screen_edges() {
        for width in [240.0, 400.0, 240.0 * 16.0 / 9.0, 560.0] {
            let parts = layout(13, width).1;
            for &(name, from) in super::super::screen_transform::ALTAR {
                let to = parts.iter().find(|part| part.model == Some(name)).unwrap().place.to;
                let scale = (width / 320.0).min((240.0 - 30.0 * 0.7) / 240.0) * CONTROL_SCALE;
                assert!((to[2] - from[2] * scale).abs() < 0.001);
                assert!((to[3] - from[3] * scale).abs() < 0.001);
                for (axis, limit, native) in [(0, width, 320.0), (1, 240.0, 240.0)] {
                    if from[axis] == 0.0 { assert!(to[axis].abs() < 0.001); }
                    if from[axis] + from[axis + 2] == native {
                        assert!((to[axis] + to[axis + 2] - limit).abs() < 0.001);
                    }
                }
            }
        }
    }

    #[test]
    fn solo_menu_places_arrows_above_hint_and_options_on_bottom() {
        for width in [240.0, 400.0, 240.0 * 16.0 / 9.0, 560.0] {
            let (_, parts) = layout(12, width);
            let find = |model: &str| parts.iter().find(|part| part.model == Some(model)).unwrap().place.to;
            let hint_scale = HINT_SCALE.min(width / 400.0);
            let hint_height = 30.0 * hint_scale;
            let baseline = 240.0 - hint_height - 2.0;

            let move_out = find("MenSgHomeBt0002");
            let practice = find("MenSgHomeBt0001");
            let altar = find("MenSgHomeBt0003");
            let idols = find("MenSgHomeBt0006");
            let gear = find("MenSgHomeBt0005");
            let back = find("MenSgHomeBt9999");
            let treasure = find("MenSgHomeBt0004");

            // Move Out is centered horizontally
            assert!(((move_out[0] + move_out[2] * 0.5) - width * 0.5).abs() < 0.001);
            // Move Out, Practice Range, Arms Altar sit right above the hint text
            assert!((move_out[1] + move_out[3] - baseline).abs() < 0.001);
            assert!((practice[1] + practice[3] - baseline).abs() < 0.001);
            assert!((altar[1] + altar[3] - baseline).abs() < 0.001);

            // Practice Range is to the left of Move Out, Arms Altar is to the right
            assert!(practice[0] < move_out[0]);
            assert!(altar[0] > move_out[0]);

            // Extra options on the bottom: Back, Idol Toss, Gear sit on the bottom edge (y + height = 240)
            assert!((back[1] + back[3] - 240.0).abs() < 0.001);
            assert!((idols[1] + idols[3] - 240.0).abs() < 0.001);
            assert!((gear[1] + gear[3] - 240.0).abs() < 0.001);

            // Back is at the bottom-left edge, Gear at the bottom-right edge
            assert!(back[0].abs() < 0.001);
            assert!((gear[0] + gear[2] - width).abs() < 0.001);
            // Idol Toss is placed to the right of Back
            assert!(idols[0] >= back[0] + back[2]);

            // Treasure Hunt is in the top-right corner
            assert!(treasure[1].abs() < 0.001);
            assert!((treasure[0] + treasure[2] - width).abs() < 0.001);
        }
    }

    #[test]
    fn options_pages_are_not_cut_at_their_boxes() {
        for index in 4..=7 {
            assert!(board_cut(index) && !drawn_cut(index), "Screen {index}'s frames reach past their boxes");
        }
        assert!(drawn_cut(8), "Volume keeps its shared Board cut");
        assert!(drawn_cut(9) && drawn_cut(10) && drawn_cut(11), "Profile and the dialogs keep their bottom-pass text");
        assert!(!part_cut(14, Some("MenEquipBt0002")), "Gear's Powers frame reaches past its box");
        assert!(part_cut(14, Some("MenEquipBt0001")), "Gear's Weapons card keeps its text at its box");
        assert!(!part_cut(20, Some("MenEquipBt0102")), "category tiles' Outlines reach past their boxes");
        assert!(part_cut(19, Some("MenEquipBt0120")), "the weapon list keeps its text at its box");
        assert!(part_cut(8, Some("MenEquipBt0002")), "Volume stays cut");
    }

    #[test]
    fn armory_text_replay_omits_glyphs_owned_by_other_controls() {
        let mut system = System::new(crate::Config::default());
        let text = 0x14000000;
        system.memory.map(text, 0x20000000, 0x1000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        let mut mouse = shown(18);
        mouse.menu_top.active = Some((18, 0, 0, false));
        mouse.menu_top.draw_order = vec![3]; // Powers list, not Autofill or the category header.
        mouse.saved = Some(Saved { regs: [0; 16], cpsr: system.cpu.cpsr, vfp: [0; 32], drawing: Drawing::MenuTop(0), moved: Vec::new() });
        system.aim_mouse = Some(mouse);
        system.cpu.regs[0] = text;
        for (point, shown) in [([20.0_f32, 12.0_f32], false), ([160.0, 40.0], true), ([200.0, 12.0], false)] {
            system.memory.write_bytes(text + 0x2C, &point[0].to_le_bytes());
            system.memory.write_bytes(text + 0x30, &point[1].to_le_bytes());
            system.cpu.regs[15] = 0x00A36FA4;
            font(&mut system);
            assert_eq!(system.cpu.regs[15], if shown { 0x00A36FA4 } else { 0x00A36FA8 });
        }
    }

    #[test]
    fn isolated_models_clip_at_the_screen_instead_of_their_hit_box() {
        let mut system = System::new(crate::Config::default());
        system.memory.map(0x14000000, 0x20000000, 0x1000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        let mut mouse = shown(12);
        mouse.widen = 1.4;
        let part = mouse.menu_top.parts[0].clone();
        mouse.menu_top.clip = Some((part.place, part.top));
        mouse.saved = Some(Saved { regs: [0; 16], cpsr: system.cpu.cpsr, vfp: [0; 32], drawing: Drawing::MenuTop(0), moved: Vec::new() });
        system.aim_mouse = Some(mouse);
        let at = 0x14000000;
        system.cpu.regs[2] = at;
        // Both the injected clip and a model's native clip must allow its animation.
        for enabled in [0, 1] {
            system.cpu.regs[1] = enabled;
            scissor(&mut system);
            assert_eq!((0..4).map(|i| read_word(&mut system, at + i * 4)).collect::<Vec<_>>(), vec![0, 0, 240, 400]);
            scissor_done(&mut system);
        }
    }

    #[test]
    fn animated_buttons_use_live_hit_tests_outside_their_nominal_boxes() {
        let mut mouse = shown(12);
        let hash = 0x38C8C232;
        // The Arms Altar mesh can pan to the left of its original box.
        let native = (150.0, 130.0);
        let top = mouse.menus_top_point(11, native.0, native.1, Some(hash)).unwrap();
        let altar = mouse.menu_top.parts.iter().position(|part| part.model == Some("MenSgHomeBt0003")).unwrap();
        assert!(!inside([top.0, top.1], mouse.menu_top.parts[altar].place.to));
        mouse.menus_top_touch(11, top.0, top.1, false);
        let candidate = mouse.menu_top.candidates.iter().position(|&(index, _)| index == altar).unwrap();
        mouse.menu_top.candidate = candidate;
        mouse.pointer = Some(mouse.menu_top.candidates[candidate].1);
        let mut system = System::new(crate::Config::default());
        system.aim_mouse = Some(mouse);
        assert!(accept_hit(&mut system, Some(hash)));
        assert!(!accept_hit(&mut system, None), "empty space must still fail the native hit test");
    }

    #[test]
    fn clicking_a_panned_button_keeps_its_unclamped_hit_coordinates() {
        let mut mouse = shown(12);
        let hash = 0x38C8C232;
        let top = mouse.menus_top_point(11, -20.0, 130.0, Some(hash)).unwrap();
        mouse.menus_top_touch(11, top.0, top.1, false);
        let candidate = mouse.menu_top.candidates.iter().position(|&(index, _)|
            mouse.menu_top.parts[index].model == Some("MenSgHomeBt0003")).unwrap();
        mouse.menu_top.candidate = candidate;
        mouse.pointer = Some(mouse.menu_top.candidates[candidate].1);
        mouse.menu_looked = Some((mouse.pointer.unwrap(), 11));
        mouse.menu_part = Some(Found { hash, name: 0, element: 0, part: 0 });
        let hit = mouse.menus_top_touch(11, top.0, top.1, true).unwrap();
        assert!((hit.0 + 20.0).abs() < 0.001 && (hit.1 - 130.0).abs() < 0.001);
        assert!(mouse.menus_top_dragging());
    }

    #[test]
    fn a_stationary_pointer_rechecks_moving_meshes_each_frame() {
        let mut mouse = shown(12);
        mouse.mouse_mode = true;
        mouse.menus_top_touch(11, 350.0, 130.0, false);
        mouse.menu_top.candidate = 2;
        refresh_hover(&mut mouse, 11);
        assert_eq!(mouse.menu_top.candidate, 0);
        mouse.menu_top.candidate = 2;
        refresh_hover(&mut mouse, 11);
        assert_eq!(mouse.menu_top.candidate, 2, "recursive hit trials stay on their candidate");
        refresh_hover(&mut mouse, 12);
        assert_eq!(mouse.menu_top.candidate, 0);
        mouse.menu_top.candidate = 2;
        mouse.menu_top.retrying_hit = true;
        refresh_hover(&mut mouse, 13);
        assert_eq!(mouse.menu_top.candidate, 2, "a slow hit query must finish before starting another");
    }

    #[test]
    fn placed_scripted_text_omits_the_same_colour_offset_copy() {
        let mut system = System::new(crate::Config::default());
        for placed in [true, false] {
            for pc in FONT_CALL {
                let mut mouse = shown(18);
                if placed {
                    mouse.menu_top.draw_order = vec![0]; // Autofill Powers.
                    mouse.saved = Some(Saved { regs: [0; 16], cpsr: system.cpu.cpsr, vfp: [0; 32], drawing: Drawing::MenuTop(0), moved: Vec::new() });
                }
                system.aim_mouse = Some(mouse);
                system.cpu.regs[15] = pc;
                font(&mut system);
                assert_eq!(system.cpu.regs[15], pc + if placed && pc == 0x00A36F74 { 4 } else { 0 },
                    "only the placed bold copy is omitted; native text and outlines keep their passes");
            }
        }
    }

    #[test]
    fn solo_entrance_detects_the_bag_before_bottom_controls_load() {
        let mut system = System::new(crate::Config::default());
        let base = 0x14000000;
        system.memory.map(base, 0x20000000, 0x10000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        let (top, bottom, current, layers, list, model, cmdl) =
            (base, base + 0x100, base + 0x200, base + 0x1000, base + 0x4000, base + 0x5000, base + 0x6000);
        for (at, value) in [(bottom, BOTTOM_SCENE), (top + 0x40, current), (bottom + 0x40, current),
            (current + SCENE_LAYERS, layers), (layers + 12 * LAYER_SIZE + 4, list),
            (list + 0x18, list + 0x100), (list + 0x1C, list + 0x104), (list + 0x100, model),
            (model + 8, cmdl), (cmdl + 4, u32::from_le_bytes(*b"CMDL")), (cmdl + 0xC, 0x100)] {
            system.memory.write_bytes(at, &value.to_le_bytes());
        }
        system.memory.write_bytes(cmdl + 0x10C, b"MenCmnUpHeart0000\0");
        let mut mouse = AimMouse { menus_on_top: true, bottom_scene: Some(bottom), ..AimMouse::default() };
        mouse.menu_screen = None;
        system.aim_mouse = Some(mouse);
        assert_eq!(scene(&mut system, top).map(|s| s.0), Some(12));
        let mouse = system.aim_mouse.as_mut().unwrap();
        mouse.menu_top.active = Some((12, 0, current, false));
        mouse.menu_top.native.push((base, vec![0; 4]));
        system.cpu.regs[1] = 0xC;
        assert!(text(&mut system, TOP_TEXT));
        assert_eq!(system.cpu.regs[15], TOP_TEXT_DRAWN, "entrance must suppress the native count");
        system.aim_mouse.as_mut().unwrap().menu_screen = Some((2, base, 0));
        system.memory.write_bytes(base + 0x8000, &[b'3', 0, b'3', 0, b'9', 0, b'0', 0]);
        system.cpu.regs[1] = base + 0x8000;
        system.cpu.regs[2] = 4;
        system.cpu.regs[15] = 0x00A36FA4;
        font(&mut system);
        assert_eq!(system.cpu.regs[15], 0x00A36FA8, "the previous Screen's input owner must not leak the count");
    }

    #[test]
    fn menuids_name_their_screen_unless_shared() {
        assert_eq!(screen_of(3), Some(12), "MENUID_HOME is the Solo hub");
        assert_eq!(screen_of(1), Some(1));
        assert_eq!(screen_of(44), None, "both Idol Tosses show under 44");
        assert_eq!(screen_of(5), None, "the Cauldron has no scripted Screen");
        assert!(menus_of(11).is_empty(), "the SpotPass dialog shows under its parent");
        for index in 0..ScreenTransform::scripted_len() {
            assert_eq!(SCREEN_MENUS[index].0, index, "the table is in Screen order");
        }
    }

    #[test]
    fn compact_heart_count_omits_the_offset_duplicate_pass() {
        let mut system = System::new(crate::Config::default());
        system.memory.map(0x14000000, 0x20000000, 0x1000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        system.memory.write_bytes(0x14000000, &[b'4', 0, b'9', 0, b'7', 0, b'8', 0]);
        let mut mouse = shown(12);
        mouse.menu_top.drawing_text = true;
        system.aim_mouse = Some(mouse);
        system.cpu.regs[1] = 0x14000000;
        system.cpu.regs[2] = 4;
        system.cpu.regs[15] = 0x00A36F74;
        font(&mut system);
        assert_eq!(system.cpu.regs[15], 0x00A36F78);
        assert!(system.aim_mouse.as_ref().unwrap().menu_top.font_back.is_none());
        system.aim_mouse.as_mut().unwrap().menu_top.drawing_text = false;
        system.cpu.regs[15] = 0x00A36FA4;
        font(&mut system);
        assert_eq!(system.cpu.regs[15], 0x00A36FA8, "native and hint passes must omit the relocated count");
        system.aim_mouse.as_mut().unwrap().menu_top.drawing_text = false;
        system.aim_mouse.as_mut().unwrap().menus_on_top = false;
        system.cpu.regs[15] = 0x00A36F74;
        font(&mut system);
        assert_eq!(system.cpu.regs[15], 0x00A36F74, "native text keeps its effect");
    }

    fn shown(index: usize) -> AimMouse {
        let mut mouse = AimMouse { menus_on_top: true, menu_screen: Some((index, 0x14050000, 10)), ..AimMouse::default() };
        mouse.menu_top.parts = layout(index, 400.0).1;
        mouse.menu_top.drawn = Some((10, index, 0x14050000, 1.0));
        mouse
    }

    #[test]
    fn projected_books_keep_only_their_own_name_and_shadow() {
        let mut system = System::new(crate::Config::default());
        system.memory.map(0x14000000, 0x20000000, 0x1000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        system.cpu.regs[0] = 0x14000000;
        for book in 0..3 {
            let mut mouse = shown(0);
            let p = mouse.menu_top.parts[book].place;
            mouse.saved = Some(Saved { regs: [0; 16], cpsr: system.cpu.cpsr, vfp: [0; 32], drawing: Drawing::MenuTop(book * 8 + 2), moved: Vec::new() });
            system.aim_mouse = Some(mouse);
            for (owner, x, y) in [(0, 50.0_f32, 120.0_f32), (1, 160.0, 120.0), (2, 270.0, 120.0), (3, 102.0, 207.0)] {
                for shadow in [0.0, 1.0] {
                    let anchor = [(p.to[0] + (x + shadow - p.from[0]) * p.to[2] / p.from[2]) * 320.0 / 400.0,
                        p.to[1] + (y + shadow - p.from[1]) * p.to[3] / p.from[3]];
                    system.memory.write_bytes(0x1400002C, &anchor[0].to_le_bytes());
                    system.memory.write_bytes(0x14000030, &anchor[1].to_le_bytes());
                    system.cpu.regs[15] = 0x00A36FA4;
                    font(&mut system);
                    assert_eq!(system.cpu.regs[15], if owner == book { 0x00A36FA4 } else { 0x00A36FA8 });
                }
            }
        }
    }

    #[test]
    fn hiding_a_moved_control_removes_input_until_it_is_shown_again() {
        let mut mouse = shown(0);
        mouse.set_editor_layer(0, [0.0, 0.0, 125.0, 190.0]);
        assert_eq!(mouse.menus_top_touch(11, 50.0, 80.0, false), Some((50.0, 80.0)));
        mouse.set_editor_layer_visible(0, false);
        assert_eq!(mouse.menus_top_touch(11, 50.0, 80.0, false), None);
        assert!(mouse.editor_navigation_controls().iter().all(|(index, _)| *index != 0));
        mouse.set_editor_layer_visible(0, true);
        assert_eq!(mouse.menus_top_touch(11, 50.0, 80.0, false), Some((50.0, 80.0)));
        assert!(mouse.editor_navigation_controls().iter().any(|(index, _)| *index == 0));
    }

    #[test]
    fn file_select_decoration_cannot_steal_moved_book_input_or_keep_a_stale_hover() {
        let mut mouse = shown(0);
        mouse.set_editor_layer(0, [20.0, 20.0, 125.0, 190.0]);
        for index in [5, 6, 7] {
            mouse.set_editor_layer(index, [0.0, 0.0, 400.0, 240.0]);
        }
        let pointer = mouse.menus_top_touch(11, 70.0, 120.0, false);
        assert_eq!(pointer, Some((50.0, 100.0)));
        mouse.menu_part = Some(Found { hash: FILE_PARTS[0], name: 0, element: 0, part: 0 });
        assert_eq!(mouse.menus_top_touch(11, 350.0, 220.0, false), None);
        assert_eq!(mouse.hovered_menu_part(), None);
    }

    #[test]
    fn turning_file_books_keep_input_outside_the_nominal_box() {
        let mut mouse = shown(0);
        mouse.set_editor_layer(2, [200.0, 20.0, 69.0, 102.0]);
        // Book III's turned cover reaches left of its original 205px box.
        let point = mouse.menu_top.parts[2].place.map_point([203.0, 85.0]);
        let native = mouse.menus_top_touch(11, point[0], point[1], false).unwrap();
        assert!((native.0 - 203.0).abs() < 0.001);
        assert!((native.1 - 85.0).abs() < 0.001);
        assert_eq!(mouse.menus_top_touch(11, 500.0, point[1], false), None);
    }

    #[test]
    fn editor_moves_a_scripted_layer() {
        let mut mouse = shown(13);
        mouse.menu_top.active = Some((13, 0x1405_0000, 0, false));
        let before = mouse.editor_layers();
        assert!(!before.is_empty());
        let mut moved = before[0].1;
        moved[0] += 7.0;
        mouse.set_editor_layer(0, moved);
        assert_eq!(mouse.editor_layers()[0].1, moved);
        assert_eq!(mouse.menu_top.overrides, vec![(13, 0, moved)]);
    }

    #[test]
    fn loading_state_keeps_host_layout_but_clears_recorded_screen() {
        let mut mouse = shown(13);
        let moved = [23.0, 42.0, 150.0, 80.0];
        mouse.set_editor_layer(0, moved);
        mouse.reset_menu_after_load();
        assert_eq!(mouse.editor_overrides(), vec![(13, 0, moved)]);
        assert!(mouse.editor_layers().is_empty());
        assert!(mouse.menu_screen.is_none());
        assert!(mouse.menu_top.drawn.is_none());
    }

    #[test]
    fn only_visible_controls_take_input_and_drags_keep_their_transform() {
        let mut mouse = shown(18);
        let part = mouse.menu_top.parts.iter().find(|part| !part.top).unwrap().place;
        let top = mapped([120.0, 80.0], part.from, part.to);
        let pointer = mouse.menus_top_touch(11, top.0, top.1, true).unwrap();
        assert!((pointer.0 - 120.0).abs() < 0.001 && (pointer.1 - 80.0).abs() < 0.001);
        assert_eq!(mouse.menus_top_touch(11, -10.0, -10.0, true), Some((0.0, 0.0)));
        assert_eq!(mouse.menus_top_touch(11, -10.0, -10.0, false), None);
        assert_eq!(mouse.menus_top_touch(11, top.0, top.1, false), Some(pointer));
        assert_eq!(mouse.menus_top_touch(11, f32::NAN, 90.0, false), None);
        assert_eq!(mouse.menus_top_touch(11, 200.0, 0.0, false), None, "preview has no controls");
        assert_eq!(mouse.menus_top_touch(20, top.0, top.1, false), None);
        mouse.widen = 1.2;
        assert!(!mouse.menus_on_top_shown(11));
        mouse.widen = 1.0;
        mouse.menu_screen = Some((11, 0x14060000, 11));
        assert!(!mouse.menus_on_top_shown(11), "a dialog invalidates the previous menu's hit map");
    }

    #[test]
    fn exact_native_pixels_survive_frontend_round_trips() {
        let mut mouse = shown(19);
        mouse.widen = 16.0 / 9.0 / (5.0 / 3.0);
        mouse.menu_top.parts = layout(19, 400.0 * mouse.widen).1;
        mouse.menu_top.drawn.as_mut().unwrap().3 = mouse.widen;
        // Every row pixel of the weapon list Control, the densest target.
        for y in 22..152 {
            let (x, top_y) = mouse.menus_top_point(11, 36.0, y as f32, None).unwrap();
            assert_eq!(mouse.menus_top_touch(11, x, top_y, false), Some((36.0, y as f32)));
        }
    }

    #[test]
    fn held_drag_keeps_initial_pixel_through_a_slow_render_poll() {
        let mut mouse = shown(19);
        mouse.menu_top.duration = 6;
        mouse.menu_button_down = true;
        mouse.pointer = Some((36.0, 84.0));
        mouse.menu_press = Some(MenuPress { pointer: (12.0, 60.0), object: 0x14050000, since: 10, dragging: Some(10) });
        assert_eq!(mouse.menu_touch(18, true), Some((12.0, 60.0)));
        assert_eq!(mouse.menu_touch(19, true), Some((36.0, 84.0)));
    }

    #[test]
    fn state_load_discards_menu_pointers_but_keeps_settings() {
        let mut mouse = shown(12);
        mouse.menu_top.grab = Some(1);
        mouse.menu_click = true;
        mouse.pointer = Some((100.0, 100.0));
        mouse.reset_menu_after_load();
        assert!(mouse.menus_on_top);
        assert!(mouse.menu_top.parts.is_empty());
        assert!(mouse.menu_screen.is_none() && mouse.pointer.is_none());
        assert!(!mouse.menu_click && !mouse.menus_on_top_shown(11));
    }

    #[test]
    fn a_load_cancels_a_held_touch_until_release() {
        let mut mouse = shown(18);
        mouse.menu_button_down = true;
        mouse.reset_menu_after_load();
        mouse.hover(Some((100.0, 100.0)));
        assert_eq!(mouse.menu_touch(11, true), None);
        assert_eq!(mouse.menu_touch(12, false), None);
        assert_eq!(mouse.menu_touch(13, true), Some((100.0, 100.0)));
    }

    #[test]
    fn foreground_materials_restore_originals_and_animated_copies() {
        let mut system = System::new(crate::Config::default());
        system.memory.map(0x14000000, 0x20000000, 0x10000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        let (model, list, runtime, original, animated) = (0x14000000, 0x14000200, 0x14000300, 0x14001000, 0x14002000);
        for (at, value) in [(model + 0x164, list), (model + 0x168, list + 4), (list, runtime),
            (model, 0x00D14AF4), (runtime, 0x00D1510C), (runtime + 8, original), (runtime + 0x14, animated)] {
            system.memory.write_bytes(at, &u32::to_le_bytes(value));
        }
        for material in [original, animated] {
            for (offset, value) in [(4, u32::from_le_bytes(*b"MTOB")), (0x118, 1), (0x11C, 0x41), (0x120, 0x00010107)] {
                system.memory.write_bytes(material + offset, &value.to_le_bytes());
            }
        }
        let mut moved = Vec::new();
        foreground_materials(&mut system, model, &mut moved);
        for material in [original, animated] {
            assert_eq!(read_word(&mut system, material + 0x118), 0);
            assert_eq!(read_word(&mut system, material + 0x11C), 0x40);
        }
        put_back(&mut system, &moved);
        for material in [original, animated] {
            assert_eq!(read_word(&mut system, material + 0x118), 1);
            assert_eq!(read_word(&mut system, material + 0x11C), 0x41);
        }
    }

    #[test]
    fn input_lives_through_a_slow_render_but_expires_after_it() {
        let mut mouse = shown(18);
        mouse.menu_top.duration = 6;
        assert!(mouse.menus_on_top_shown(18));
        assert!(!mouse.menus_on_top_shown(20));
    }

    /// a title drawing every other refresh, as the pacer has it while it
    /// decides a picture in between, and the console's frame time.
    fn showing_a_picture_in_between(system: &mut System) -> std::time::Duration {
        use std::time::Duration;
        let frame = Duration::from_secs_f64(2.0 / 59.83);
        system.pacing.on = true;
        system.pacing.refreshes = 10;
        system.pacing.new_at = 9;
        system.pacing.between = true;
        system.pacing.older_host_time = Some(Duration::ZERO);
        system.pacing.newer_host_time = Some(frame);
        frame
    }

    /// every Scripted Menu's Hovers and clicks find the same Controls with
    /// High Refresh Mode on and off: the parts were seated on the newest
    /// real frame, and hit-testing never reads a picture in between.
    #[test]
    fn menu_hovers_and_clicks_read_the_same_real_placements_with_the_mode_on_and_off() {
        let mut off = System::new(crate::Config::default());
        let mut on = System::new(crate::Config::default());
        let frame = showing_a_picture_in_between(&mut on);
        assert_eq!(off.picture_kind_at_host_time(frame / 2), crate::PictureKind::Real);
        assert!(matches!(on.picture_kind_at_host_time(frame / 2), crate::PictureKind::InBetween(t) if (t - 0.5).abs() < 1e-5));

        let mut checked = 0;
        for index in 0..ScreenTransform::scripted_len() {
            off.aim_mouse = Some(shown(index));
            on.aim_mouse = Some(shown(index));
            let places: Vec<_> = {
                let menu = &on.aim_mouse.as_ref().unwrap().menu_top;
                menu.parts.iter()
                    .filter(|part| !part.top && menu.enabled(part) && part.model.and_then(button_hash).is_some())
                    .map(|part| part.place)
                    .collect()
            };
            for place in places {
                let top = place.map_point([place.from[0] + place.from[2] * 0.5, place.from[1] + place.from[3] * 0.5]);
                for pressed in [false, true] {
                    let off_hit = off.aim_mouse.as_mut().unwrap().menus_top_touch(11, top[0], top[1], pressed);
                    let on_hit = on.aim_mouse.as_mut().unwrap().menus_top_touch(11, top[0], top[1], pressed);
                    assert_eq!(on_hit, off_hit, "screen {index} at {top:?}, pressed {pressed}");
                    assert_eq!(
                        on.aim_mouse.as_ref().unwrap().menu_top.candidates,
                        off.aim_mouse.as_ref().unwrap().menu_top.candidates,
                        "screen {index} candidate Control at {top:?}",
                    );
                    assert!(off_hit.is_some(), "screen {index} has a Control at its part's centre");
                    checked += 1;
                }
            }
        }
        assert!(checked > 26, "every Scripted Menu with a Control was checked, {checked} hits");
    }
}

// Collision solving uses a shared, unscaled canvas. Each rendered button's
// translation is expressed in that canvas; the common artwork scale cancels.
struct PhysicsCall {
    return_pc: u32,
    bodies: Vec<(u32, [f32; 2])>,
    bounds: Vec<(u32, Vec<u8>)>,
}

fn collision_delta(system: &mut System, body: u32) -> Option<([f32; 2], f32)> {
    if body == 0 || !system.memory.is_readable(body, 0xdc) { return None; }
    let part = read_word(system, body + 0x50);
    let name_at = read_word(system, part + 0x24);
    let name = c_string(system, name_at);
    let place = system.aim_mouse.as_ref()?.menu_top.parts.iter()
        .find(|part| !part.top && part.model == Some(name.as_str()))?.place;
    let scale = place.to[2] / place.from[2];
    Some(([(place.to[0] - place.from[0] * scale) / (10.0 * scale),
        -(place.to[1] - place.from[1] * scale) / (10.0 * scale)], scale))
}

fn shift_collision_body(system: &mut System, body: u32, delta: [f32; 2]) {
    let shift = |system: &mut System, at: u32| {
        for axis in 0..2 {
            let at = at + axis as u32 * 4;
            let value = f32::from_bits(read_word(system, at)) + delta[axis];
            system.memory.write_bytes(at, &value.to_le_bytes());
        }
    };
    shift(system, body + 0x68);
    shift(system, body + 0x74);
    let mut vertex = read_word(system, body + 0x58);
    for _ in 0..128 {
        if vertex == 0 || !system.memory.is_readable(vertex, 0x4c) { break; }
        shift(system, vertex + 0x1c);
        shift(system, vertex + 0x28);
        vertex = read_word(system, vertex + 0x48);
    }
    let mut auxiliary = read_word(system, body + 0x4c);
    for _ in 0..128 {
        if auxiliary == 0 || !system.memory.is_readable(auxiliary, 0x4c) { break; }
        shift(system, auxiliary + 0x28);
        auxiliary = read_word(system, auxiliary + 0x48);
    }
    let mut joint = read_word(system, body + 0x64);
    for _ in 0..128 {
        if joint == 0 || !system.memory.is_readable(joint, 0x70) { break; }
        shift(system, joint + 0x58);
        joint = read_word(system, joint + 0x6c);
    }
}

pub(super) fn physics(system: &mut System, pc: u32) {
    // Native-to-interpreter calls consume the entry instruction. Stop
    // after the register push, where all arguments are still unchanged.
    let follow_drag = pc == 0x0025EF84;
    let return_pc = match pc { 0x002A3374 | 0x002A3470 | 0x002A34C8 => pc + 4, _ => system.cpu.regs[14] };
    let pc = match pc {
        0x0025EF84 => 0x00A38510,
        0x00A390F0 => 0x00A390EC, 0x00A3872C => 0x00A38728,
        0x00A38514 => 0x00A38510, 0x002A3374 | 0x002A3470 => 0x002F0770,
        0x002A34C8 => 0x002F0DEC,
        0x00A390EC | 0x00A38728 | 0x00A38510 | 0x002F0770 | 0x002F0DEC => return,
        _ => pc,
    };
    if matches!(pc, 0x00A390EC | 0x00A38728 | 0x00A38510) {
        log::debug!("drag hook {pc:08X}, screen {:?}", system.aim_mouse.as_ref().and_then(|mouse| mouse.menu_screen));
        let Some(mouse) = system.aim_mouse.as_ref().filter(|mouse| mouse.menus_on_top
            && mouse.menu_screen.is_some() && !mouse.menu_top.parts.is_empty()) else { return; };
        let host = if mouse.menu_top.grab.is_some() {
            if pc == 0x00A390EC || pc == 0x00A38728 {
                mouse.menu_press.as_ref().map(|press| press.pointer).or(mouse.pointer)
            } else { mouse.pointer }
        } else { None };
        if follow_drag && (host.is_none() || !mouse.menu_button_down) { return; }
        let (input, at) = (system.cpu.regs[0], system.cpu.regs[1]);
        let point = host.unwrap_or_else(|| (f32::from_bits(read_word(system, at)), f32::from_bits(read_word(system, at + 4))));
        if pc == 0x00A390EC {
            system.memory.write_bytes(at, &point.0.to_le_bytes());
            system.memory.write_bytes(at + 4, &point.1.to_le_bytes());
            return;
        }
        let body = read_word(system, input + 0x5fc);
        let joint = read_word(system, input + 0x600);
        if body == 0 || joint == 0 { return; }
        if collision_delta(system, body).is_none() { return; }
        let target = [(point.0 - 160.0) * 0.1, (120.0 - point.1) * 0.1];
        log::debug!("drag target {point:?} => {target:?}, body {body:08X}");
        let previous = system.aim_mouse.as_ref().unwrap().menu_top.drag_target;
        if pc == 0x00A38510 {
            if let Some((_, previous)) = previous.filter(|(old_joint, _)| *old_joint == joint) {
                for axis in 0..2 {
                    system.memory.write_bytes(body + 0x98 + axis as u32 * 4, &(previous[axis] - target[axis]).to_le_bytes());
                }
            }
        }
        system.aim_mouse.as_mut().unwrap().menu_top.drag_target = Some((joint, target));
        for axis in 0..2 {
            if pc == 0x00A38728 {
                let offset = target[axis] - f32::from_bits(read_word(system, body + 0x74 + axis as u32 * 4));
                system.memory.write_bytes(joint + 0xc + axis as u32 * 4, &offset.to_le_bytes());
            }
            for field in [0x5d8, 0x620, 0x62c] {
                system.memory.write_bytes(input + field + axis as u32 * 4, &target[axis].to_le_bytes());
            }
        }
        return;
    }
    if let Some(call) = system.aim_mouse.as_mut().and_then(|mouse|
        mouse.menu_top.physics.take_if(|call| call.return_pc == pc)) {
        for (body, delta) in call.bodies { shift_collision_body(system, body, [-delta[0], -delta[1]]); }
        put_back(system, &call.bounds);
        return;
    }
    if !matches!(pc, 0x002F0770 | 0x002F0DEC) { return; }
    let Some(mouse) = system.aim_mouse.as_ref().filter(|mouse| mouse.menus_on_top
        && mouse.menu_top.physics.is_none() && mouse.menu_screen.is_some()) else { return };
    let width = 400.0 * mouse.widen;
    let first = system.cpu.regs[1];
    let Some((delta, scale)) = collision_delta(system, first) else { return };
    let mut call = PhysicsCall { return_pc, bodies: vec![(first, delta)], bounds: Vec::new() };
    if pc == 0x002F0770 {
        let second = system.cpu.regs[2];
        let Some((delta, other_scale)) = collision_delta(system, second) else { return };
        if (scale - other_scale).abs() > 0.0001 { return; }
        call.bodies.push((second, delta));
    } else {
        let world = system.cpu.regs[0];
        let count = read_word(system, world + 4);
        if count > 32 { return; }
        for edge in 0..count {
            for offset in [0x3c, 0x48] {
                let at = world + edge * 0x24 + offset;
                let x = f32::from_bits(read_word(system, at));
                let y = f32::from_bits(read_word(system, at + 4));
                let values = [(x + 16.0) * width / (320.0 * scale) - 16.0,
                    y / scale + 12.0 - 12.0 / scale];
                let bytes: Vec<u8> = values.into_iter().flat_map(f32::to_le_bytes).collect();
                change(system, &mut call.bounds, at, &bytes);
            }
        }
    }
    for &(body, delta) in &call.bodies { shift_collision_body(system, body, delta); }
    system.aim_mouse.as_mut().unwrap().menu_top.physics = Some(call);
}

#[cfg(test)]
mod collision_tests {
    use super::*;

    #[test]
    fn explicit_navigation_uses_native_names_and_restores_defaults() {
        let mut system = System::new(crate::Config::default());
        system.memory.map(0x14000000, 0x20000000, 0x1000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        let mut mouse = AimMouse { menus_on_top: true, menu_screen: Some((2, 1, 0)), ..AimMouse::default() };
        mouse.menu_top.parts = layout(2, 400.0).1;
        mouse.set_editor_navigation_link(7, NavigationDirection::Right, Some(2));
        system.aim_mouse = Some(mouse);
        for (node, next, hash) in [(0x14000100_u32, 0x14000300_u32, 0x2E49363F_u32),
            (0x14000300, 0x14000100, 0xE250C577)] {
            system.memory.write_bytes(node + 4, &next.to_le_bytes());
            system.memory.write_bytes(node + 0xc, &hash.to_le_bytes());
        }
        system.cpu.regs[0] = 0x14000100;
        system.cpu.regs[1] = 5;
        navigate(&mut system, 0x003506C4);
        assert_eq!((system.cpu.regs[0], system.cpu.regs[1]), (0x14000300, 9));
        system.aim_mouse.as_mut().unwrap().set_editor_navigation_link(7, NavigationDirection::Right, Some(7));
        system.cpu.regs[0] = 0x14000100;
        system.cpu.regs[1] = 5;
        navigate(&mut system, 0x003506C4);
        assert_eq!((system.cpu.regs[0], system.cpu.regs[1]), (0x14000100, 9));
        let mouse = system.aim_mouse.as_mut().unwrap();
        mouse.set_editor_navigation_link(7, NavigationDirection::Right, None);
        system.cpu.regs[0] = 0x14000100;
        system.cpu.regs[1] = 5;
        navigate(&mut system, 0x003506C4);
        assert_eq!((system.cpu.regs[0], system.cpu.regs[1]), (0x14000100, 5));
    }

    #[test]
    fn category_links_keep_back_return_and_locked_tiles() {
        let mut system = System::new(crate::Config::default());
        system.memory.map(0x14000000, 0x20000000, 0x1000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        let object = 0x14000100;
        let mut mouse = AimMouse { menus_on_top: true, menu_screen: Some((20, object, 0)), ..AimMouse::default() };
        mouse.menu_top.parts = layout(20, 400.0).1;
        mouse.set_editor_navigation_link(CATEGORY_BACK, NavigationDirection::Up, Some(0));
        mouse.set_editor_navigation_link(0, NavigationDirection::Right, Some(1));
        mouse.set_editor_navigation_link(0, NavigationDirection::Down, Some(0));
        system.aim_mouse = Some(mouse);
        let step = |system: &mut System, from: u32, table: u32| {
            system.memory.write_bytes(object + CATEGORY, &from.to_le_bytes());
            system.cpu.regs[4] = u32::MAX;
            system.cpu.regs[5] = object;
            system.cpu.regs[6] = 0;
            system.cpu.regs[7] = table;
            system.cpu.regs[15] = CATEGORY_STEP;
            navigate(system, CATEGORY_STEP);
            (system.cpu.regs[4], system.cpu.regs[15])
        };
        // Back returns to its saved tile natively, so the link replaces that tile.
        assert_eq!(step(&mut system, CATEGORY_BACK as u32, 0x00CA2730), (0, CATEGORY_TAKE));
        assert_eq!(read_word(&mut system, object + CATEGORY_BEFORE), 0);
        // A tile the save has no weapons for stays unreachable.
        assert_eq!(step(&mut system, 0, 0x00CA2AA0), (u32::MAX, CATEGORY_STEP));
        assert_eq!(step(&mut system, 0, 0x00CA2E10), (u32::MAX, CATEGORY_NONE));
        assert_eq!(step(&mut system, 0, 0x00CA3180), (u32::MAX, CATEGORY_STEP));
    }

    #[test]
    fn main_board_collision_and_drag_follow_edited_placements() {
        let mut system = System::new(crate::Config::default());
        system.memory.map(0x14000000, 0x20000000, 0x10000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        let mut mouse = AimMouse { menus_on_top: true, menu_screen: Some((2, 1, 0)), ..AimMouse::default() };
        mouse.menu_top.parts = layout(2, 400.0).1;
        let index = mouse.menu_top.parts.iter().position(|part| part.model == Some("MenTopBt0001")).unwrap();
        let mut place = mouse.menu_top.parts[index].place;
        place.to[0] += 100.0;
        mouse.set_editor_layer(index, place.to);
        system.aim_mouse = Some(mouse);
        let (body, part, name_at, joint, input, point) = (0x14000100, 0x14001000, 0x14002000, 0x14003000, 0x14004000, 0x14005000);
        system.memory.write_bytes(body + 0x50, &u32::to_le_bytes(part));
        system.memory.write_bytes(part + 0x24, &u32::to_le_bytes(name_at));
        system.memory.write_bytes(name_at, b"MenTopBt0001\0");
        system.memory.write_bytes(body + 0x74, &2.0_f32.to_le_bytes());
        system.cpu.regs[1] = body;
        system.cpu.regs[2] = body;
        system.cpu.regs[14] = 0x002A34CC;
        physics(&mut system, 0x002A34C8);
        let delta = (place.to[0] - place.from[0] * place.to[2] / place.from[2]) / (10.0 * place.to[2] / place.from[2]);
        assert!((f32::from_bits(read_word(&mut system, body + 0x74)) - 2.0 - delta).abs() < 0.001,
            "collision must use the edited Main Menu placement");
        physics(&mut system, 0x002A34CC);
        system.memory.write_bytes(input + 0x5fc, &u32::to_le_bytes(body));
        system.memory.write_bytes(input + 0x600, &u32::to_le_bytes(joint));
        system.memory.write_bytes(point, &681.0_f32.to_le_bytes());
        system.memory.write_bytes(point + 4, &122.0_f32.to_le_bytes());
        system.cpu.regs[0] = input;
        system.cpu.regs[1] = point;
        physics(&mut system, 0x00A3872C);
        assert!((f32::from_bits(read_word(&mut system, input + 0x5d8)) - 52.1).abs() < 0.001,
            "Main Menu drags must extend beyond the native bottom screen");
    }
    #[test]
    fn collision_space_matches_artwork_and_preserves_solver_motion() {
        let mut system = System::new(crate::Config::default());
        system.memory.map(0x14000000, 0x20000000, 0x10000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        system.aim_mouse = Some(AimMouse { menus_on_top: true, menu_screen: Some((12, 1, 0)), ..AimMouse::default() });
        system.aim_mouse.as_mut().unwrap().menu_top.parts = layout(12, 400.0).1;
        for (body, part, name_at, name) in [(0x14000100, 0x14001000, 0x14002000, "MenSgHomeBt0001"), (0x14000300, 0x14001200, 0x14002200, "MenSgHomeBt0003")] {
            system.memory.write_bytes(body + 0x50, &u32::to_le_bytes(part));
            system.memory.write_bytes(part + 0x24, &u32::to_le_bytes(name_at));
            system.memory.write_bytes(name_at, name.as_bytes());
            system.memory.write_bytes(body + 0x74, &2.0_f32.to_le_bytes());
        }
        system.memory.write_bytes(0x14000164, &0x14003000_u32.to_le_bytes());
        system.memory.write_bytes(0x14003058, &2.0_f32.to_le_bytes());
        system.cpu.regs[1] = 0x14000100;
        system.cpu.regs[2] = 0x14000300;
        system.cpu.regs[14] = 0x002A3378;
        physics(&mut system, 0x002A3374);
        for body in [0x14000100, 0x14000300] {
            let (delta, _) = collision_delta(&mut system, body).unwrap();
            assert!((f32::from_bits(read_word(&mut system, body + 0x74)) - 2.0 - delta[0]).abs() < 0.0001);
        }
        let moved = f32::from_bits(read_word(&mut system, 0x14000174)) + 3.0;
        system.memory.write_bytes(0x14000174, &moved.to_le_bytes());
        physics(&mut system, 0x002A3378);
        assert!((f32::from_bits(read_word(&mut system, 0x14000174)) - 5.0).abs() < 0.0001);
        assert!((f32::from_bits(read_word(&mut system, 0x14000374)) - 2.0).abs() < 0.0001);
        assert!(system.aim_mouse.as_ref().unwrap().menu_top.physics.is_none());
        assert!((f32::from_bits(read_word(&mut system, 0x14003058)) - 2.0).abs() < 0.0001,
            "drag/spring anchors must return to native space too");
        let input = 0x14004000;
        system.memory.write_bytes(input + 0x5fc, &0x14000100_u32.to_le_bytes());
        system.memory.write_bytes(input + 0x600, &0x14003000_u32.to_le_bytes());
        system.memory.write_bytes(0x14005000, &681.0_f32.to_le_bytes());
        system.memory.write_bytes(0x14005004, &122.0_f32.to_le_bytes());
        system.cpu.regs[0] = input;
        system.cpu.regs[1] = 0x14005000;
        physics(&mut system, 0x00A3872C);
        assert!((f32::from_bits(read_word(&mut system, input + 0x5d8)) - 52.1).abs() < 0.0001,
            "a long PC drag must reach beyond the original 320-pixel screen");
        assert!((f32::from_bits(read_word(&mut system, input + 0x5dc)) + 0.2).abs() < 0.0001);
        system.memory.write_bytes(0x14005000, &(-120.0_f32).to_le_bytes());
        physics(&mut system, 0x00A38514);
        assert!((f32::from_bits(read_word(&mut system, input + 0x5d8)) + 28.0).abs() < 0.0001,
            "the drag target must also support the expanded canvas to the left");
        let mouse = system.aim_mouse.as_mut().unwrap();
        mouse.menu_top.grab = Some(0);
        mouse.menu_button_down = true;
        mouse.pointer = Some((900.0, 80.0));
        system.memory.write_bytes(0x14005000, &319.0_f32.to_le_bytes());
        physics(&mut system, 0x0025EF84);
        assert!((f32::from_bits(read_word(&mut system, input + 0x5d8)) - 74.0).abs() < 0.0001,
            "held drags must keep updating when the physical touch has reached its edge");
    }
}
