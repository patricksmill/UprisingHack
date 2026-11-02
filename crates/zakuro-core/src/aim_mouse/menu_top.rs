//! Scripted menus drawn by the game's scene renderer, with the same boxes
//! used to place their controls and to undo that placement for input.

use super::*;

pub(super) const SCISSOR: u32 = 0x009164A8;
pub(super) const SCISSOR_DONE: u32 = 0x00916580;
pub(super) const BOTTOM_TEXT: u32 = 0x007C912C;
pub(super) const BOTTOM_TRANSLUCENT: u32 = 0x007C90F8;
pub(super) const START: u32 = 0x0072A050;
pub(super) const FINISHED: u32 = 0x0072A0B4;
pub(super) const FONT_CALL: [u32; 7] = [0x00A36E64, 0x00A36EC8, 0x00A36EE4, 0x00A36F00, 0x00A36F1C, 0x00A36F74, 0x00A36FA4];
pub(super) const FONT_DONE: [u32; 7] = [0x00A36E68, 0x00A36ECC, 0x00A36EE8, 0x00A36F04, 0x00A36F20, 0x00A36F78, 0x00A36FA8];

#[derive(Default)]
pub(super) struct MenuTop {
    drawn: Option<(u64, usize, u32, f32)>,
    idol_dialog: bool,
    physics: Option<PhysicsCall>,
    drag_target: Option<(u32, [f32; 2])>,
    parts: Vec<Part>,
    draw_order: Vec<usize>,
    native: Vec<(u32, Vec<u8>)>,
    clip: Option<(CauldronPlace, bool)>,
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
    cauldron_font: bool,
    font_back: Option<(u32, [u8; 16])>,
    text_parent: Option<Box<Saved>>,
    drawing_text: bool,
    pub(super) grab: Option<usize>,
    pub(super) blocked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IdolGroup { Counters, Back, Launcher, Dialog }

impl IdolGroup {
    fn contains(self, name: &str) -> bool {
        match self {
            Self::Counters => matches!(name, "MenColVisionBt0005" | "MenColVisionBt0007"),
            Self::Dialog => name == "MenCmnWindowBt0000",
            Self::Back => name == "MenColVisionBt9999",
            Self::Launcher => matches!(name, "MenColVisionBt0000" | "MenColVisionBt0001" | "MenColVisionBt0003"),
        }
    }
}

#[derive(Clone)]
struct Part {
    place: CauldronPlace,
    top: bool,
    model: Option<&'static str>,
    idol: Option<IdolGroup>,
    chapter: bool,
}

impl Part {
    fn hit(&self, point: [f32; 2]) -> bool {
        if !inside(point, self.place.to) { return false; }
        if self.idol == Some(IdolGroup::Dialog) {
            let (x, y) = mapped(point, self.place.to, self.place.from);
            return inside([x, y], [15.0, 20.0, 290.0, 200.0]);
        }
        true
    }
}

impl MenuTop {
    fn part(&self, sequence: usize) -> Option<&Part> {
        self.parts.get(self.draw_order.get(sequence).copied().unwrap_or(sequence))
    }
}

fn draw_order(system: &mut System, bottom: u32, parts: &[Part]) -> Vec<usize> {
    let scene = read_word(system, bottom + 0x40);
    let layers = read_word(system, scene + SCENE_LAYERS);
    let mut names = Vec::new();
    for layer in BOTTOM_LAYERS {
        let list = read_word(system, layers + layer * LAYER_SIZE + 4);
        if list == 0 { continue; }
        let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
        if end >= start && end - start <= 0x1000 {
            for at in (start..end).step_by(4) { let model = read_word(system, at); names.push(model_name(system, model)); }
        }
    }
    let mut order: Vec<usize> = (0..parts.len()).collect();
    order.sort_by_key(|&index| {
        let part = &parts[index];
        if part.top { return if part.model.is_some() { 0 } else { usize::MAX }; }
        1 + part.model.and_then(|name| names.iter().position(|model| model == name)).unwrap_or(0)
    });
    order
}

fn fitted(from: [f32; 4], area: [f32; 4]) -> CauldronPlace {
    let scale = (area[2] / from[2]).min(area[3] / from[3]);
    let (width, height) = (from[2] * scale, from[3] * scale);
    CauldronPlace { from, to: [area[0] + (area[2] - width) * 0.5, area[1] + (area[3] - height) * 0.5, width, height] }
}

fn inside(point: [f32; 2], rect: [f32; 4]) -> bool {
    point.into_iter().enumerate().all(|(axis, value)| value.is_finite() && value >= rect[axis] && value < rect[axis] + rect[axis + 2])
}

fn mapped(point: [f32; 2], from: [f32; 4], to: [f32; 4]) -> (f32, f32) {
    (to[0] + (point[0] - from[0]) * to[2] / from[2], to[1] + (point[1] - from[1]) * to[3] / from[3])
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
        "MenSgHomeBt0001" => 0x94CE5E14, "MenSgHomeBt0002" => 0x79BCE02D,
        "MenSgHomeBt0003" => 0x38C8C232, "MenSgHomeBt0004" => 0xED263933,
        "MenSgHomeBt0005" => 0x71966FCC, "MenSgHomeBt0006" => 0x4F73A4C0,
        "MenTopBt0000" => 0xFD90A21D, "MenTopBt0001" => 0x2E49363F,
        "MenTopBt0002" => 0xE2DCC735, "MenTopBt0003" => 0xE250C577,
        "MenTopBt0004" => 0x5407F273, "MenTopBt0005" => 0xD929D345,
        "MenTopBt0006" => 0x9A07E025, "MenTopBt0007" => 0x4A7AE3E3,
        "MenAlterBt0001" => 0x1B80430F, "MenAlterBt0002" => 0xC59BD435,
        "MenAlterBt0003" => 0xE43D88E9, "MenAlterBt0004" => 0x71966FCC,
        "MenSgHomeBt9999" | "MenAlterBt9999" => 0x9A07E025,
        _ => return None,
    })
}

/// Hubs have individual animated models; a list or board stays one group.
const SOLO: &[(&str, [f32; 4])] = &[
    ("MenSgHomeBt0001", [0.0, 92.0, 88.0, 112.0]),
    ("MenSgHomeBt0003", [188.0, 68.0, 132.0, 124.0]),
    ("MenSgHomeBt0004", [226.0, 0.0, 94.0, 76.0]),
    ("MenSgHomeBt0005", [178.0, 160.0, 142.0, 80.0]),
    ("MenSgHomeBt0006", [84.0, 174.0, 96.0, 66.0]),
    ("MenSgHomeBt9999", [0.0, 194.0, 48.0, 46.0]),
    ("MenSgHomeBt0002", [12.0, 0.0, 242.0, 164.0]),
];
const ALTAR: &[(&str, [f32; 4])] = &[
    ("MenAlterBt0002", [140.0, 28.0, 180.0, 132.0]),
    ("MenAlterBt0003", [0.0, 88.0, 206.0, 122.0]),
    ("MenAlterBt0004", [130.0, 150.0, 190.0, 90.0]),
    ("MenAlterBt9999", [0.0, 192.0, 58.0, 48.0]),
    ("MenAlterBt0001", [0.0, 0.0, 198.0, 108.0]),
];
const MAIN: &[(&str, [f32; 4])] = &[
    ("MenTopBt0000", [260.0, 178.0, 60.0, 62.0]),
    ("MenTopBt0002", [120.0, 82.0, 200.0, 98.0]),
    ("MenTopBt0003", [0.0, 82.0, 130.0, 98.0]),
    ("MenTopBt0004", [198.0, 0.0, 122.0, 86.0]),
    ("MenTopBt0005", [40.0, 176.0, 188.0, 64.0]),
    ("MenTopBt0006", [4.0, 190.0, 62.0, 50.0]),
    ("MenTopBt0007", [220.0, 178.0, 50.0, 62.0]),
    ("MenTopBt0001", [0.0, 0.0, 210.0, 100.0]),
];
const OPTIONS: &[(&str, [f32; 4])] = &[
    ("MenOptionTopBt0001", [58.0, 0.0, 262.0, 104.0]),
    ("MenOptionTopBt0002", [0.0, 78.0, 266.0, 102.0]),
    ("MenOptionTopBt0003", [100.0, 148.0, 220.0, 92.0]),
    ("MenOptionTopBt9999", [0.0, 198.0, 50.0, 42.0]),
];
const PRACTICE: &[(&str, [f32; 4])] = &[
    ("MenTrymissionBt0002", [46.0, 94.0, 274.0, 122.0]),
    ("MenTrymissionBt0003", [46.0, 192.0, 274.0, 48.0]),
    ("MenTrymissionBt9999", [0.0, 196.0, 50.0, 44.0]),
    ("MenTrymissionBt0001", [0.0, 0.0, 294.0, 116.0]),
];
const VAULT: &[(&str, [f32; 4])] = &[
    ("MenCollectionTopBt0002", [168.0, 0.0, 130.0, 110.0]),
    ("MenCollectionTopBt0003", [100.0, 184.0, 156.0, 56.0]),
    ("MenCollectionTopBt0005", [0.0, 98.0, 110.0, 92.0]),
    ("MenCollectionTopBt0006", [108.0, 98.0, 110.0, 92.0]),
    ("MenCollectionTopBt0007", [212.0, 98.0, 108.0, 92.0]),
    ("CmnBack0000", [0.0, 196.0, 50.0, 44.0]),
    ("MenCollectionTopBt0009", [260.0, 194.0, 60.0, 46.0]),
    ("MenCollectionTopBt0001", [0.0, 0.0, 170.0, 110.0]),
];

fn hint(index: usize) -> Option<[f32; 4]> {
    if let Some(kind) = index.checked_sub(MENU_SCREENS.len()) {
        return match kind {
            0 | 3 | 4 => Some([0.0, 210.0, 400.0, 30.0]),
            5 => Some([0.0, 190.0, 400.0, 50.0]),
            6 => Some([0.0, 170.0, 400.0, 70.0]),
            _ => None,
        };
    }
    match index {
        2..=6 | 8..=13 | 17 | 19 => Some([0.0, 210.0, 400.0, 30.0]),
        14 | 20 => Some([0.0, 0.0, 400.0, 30.0]),
        21 => Some([0.0, 0.0, 400.0, 108.0]),
        22 | 25 => Some([0.0, 170.0, 400.0, 70.0]),
        23 => Some([0.0, 190.0, 400.0, 50.0]),
        24 => Some([0.0, 200.0, 400.0, 40.0]),
        _ => None,
    }
}

// Relative to the original one-screen menu sizes. Input uses these same boxes.
const CONTROL_SCALE: f32 = 0.6;
const HINT_SCALE: f32 = 0.7 * CONTROL_SCALE;

fn has_details(index: usize) -> bool {
    matches!(index, 0 | 7 | 14..=20) || index >= MENU_SCREENS.len()
}

fn layout(index: usize, width: f32) -> (CauldronPlace, Vec<Part>) {
    if index == 1 {
        let native = CauldronPlace { from: [0.0, 0.0, 400.0, 240.0], to: [0.0, 0.0, width, 240.0] };
        let scale = 0.48_f32.min((width - 16.0) / 400.0);
        let details = CauldronPlace { from: native.from, to: [(width - 400.0 * scale) * 0.5, 4.0, 400.0 * scale, 240.0 * scale] };
        let controls = CauldronPlace { from: [0.0, 0.0, 320.0, 240.0], to: [(width - 320.0 * scale) * 0.5, 122.0, 320.0 * scale, 240.0 * scale] };
        return (native, vec![
            Part { chapter: true, idol: None, top: true, model: None, place: details },
            Part { chapter: true, idol: None, top: false, model: None, place: controls },
        ]);
    }
    let hint = hint(index);
    let mut area = [0.0, 0.0, width, 240.0];
    if let Some([_, y, _, height]) = hint {
        area[1] = if y == 0.0 { height * HINT_SCALE } else { 0.0 };
        area[3] -= height * HINT_SCALE;
    }
    let groups = match index { 2 => MAIN, 3 => OPTIONS, 12 => SOLO, 13 => ALTAR, 21 => PRACTICE, 24 => VAULT, _ => &[] };
    let details = has_details(index);
    // The original top scene is the backdrop, never a thumbnail or a
    // cropped panel. Text is replayed separately without any scene meshes.
    let mut native = CauldronPlace { from: [0.0, 0.0, 400.0, 240.0], to: [0.0, 0.0, width, 240.0] };
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
    if index == 12 {
        parts.push(Part { chapter: false, idol: None, top: true, model: Some("MenCmnUpHeart0000"), place: fitted([300.0, 138.0, 100.0, 78.0], [0.0, 0.0, 70.0 * CONTROL_SCALE, 55.0 * CONTROL_SCALE]) });
    }
    if !groups.is_empty() { controls = [0.0, 0.0, width, 240.0]; }
    if matches!(index, 22 | 25) {
        let counter_scale = 0.7_f32.min((width - 16.0) / 160.0);
        let launcher_scale = 0.68_f32.min(width * 0.6 / 280.0);
        for (group, from, to) in [
            (IdolGroup::Counters, [0.0, 0.0, 160.0, 72.0], [0.0, 14.0, 160.0 * counter_scale, 72.0 * counter_scale]),
            (IdolGroup::Back, [0.0, 190.0, 60.0, 50.0], [8.0, 202.0, 36.0, 30.0]),
            (IdolGroup::Launcher, [40.0, 40.0, 240.0, 110.0],
                [width - 8.0 - 240.0 * launcher_scale, 62.0, 240.0 * launcher_scale, 110.0 * launcher_scale]),
        ] {
            parts.push(Part { chapter: false, idol: Some(group), top: false, model: None, place: CauldronPlace { from, to } });
        }
    } else if groups.is_empty() {
        parts.push(Part { chapter: false, idol: None, top: false, model: None, place: {
            let mut place = fitted([0.0, 0.0, 320.0, 240.0], controls);
            for axis in 0..2 {
                place.to[axis] += place.to[axis + 2] * (1.0 - CONTROL_SCALE) * 0.5;
                place.to[axis + 2] *= CONTROL_SCALE;
            }
            place
        } });
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
            parts.push(Part { chapter: false, idol: None, top: false, model: Some(name), place: CauldronPlace { from, to } });
        }
    }
    if let Some(mut from) = hint {
        let scale = if matches!(index, 22 | 25) {
            // Keep both complete hint lines readable, with room for Back
            // in narrow windows and for the dialog above the text.
            from = [0.0, 175.0, 400.0, 45.0];
            0.6_f32.min((width - 88.0) / from[2])
        } else { HINT_SCALE.min(width / from[2]) };
        let height = from[3] * scale;
        parts.push(Part { chapter: false, idol: None, top: true, model: None, place: CauldronPlace {
            from, to: [(width - from[2] * scale) * 0.5, if from[1] == 0.0 { 0.0 } else { 240.0 - height }, from[2] * scale, height],
        } });
    }
    (native, parts)
}

fn layout_with_dialog(index: usize, width: f32, idol_dialog: bool) -> (CauldronPlace, Vec<Part>) {
    let (native, mut parts) = layout(if idol_dialog { 22 } else { index }, width);
    if idol_dialog {
        let place = fitted([0.0, 0.0, 320.0, 240.0], [8.0, 48.0, width - 16.0, 180.0]);
        parts.insert(parts.len() - 1, Part { chapter: false, idol: Some(IdolGroup::Dialog), top: false, model: None, place });
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

impl AimMouse {
    pub(super) fn menu_age(&self) -> u64 { 3 + if self.menus_on_top { self.menu_top.duration } else { 0 } }

    pub fn menus_top_dragging(&self) -> bool { self.menu_top.grab.is_some() }

    pub fn clear_menus_top_pointer(&mut self) {
        self.menu_top.pointer = None;
        self.menu_top.candidates.clear();
        self.menu_top.hit_frame = None;
    }

    pub fn menus_on_top_shown(&self, frame: u64) -> bool {
        let age = 3 + self.menu_top.duration;
        self.menus_on_top && self.menu_top.drawn.is_some_and(|(drawn, index, object, widen)|
            frame >= drawn && frame <= drawn + age && widen == self.widen
                && if index >= MENU_SCREENS.len() {
                    self.menu_screen.is_none_or(|(_, _, seen)| seen <= drawn)
                } else {
                    self.menu_screen.is_some_and(|(active, owner, seen)| active == index && owner == object && frame >= seen
                        && (self.menu_top.passthrough || frame <= seen + age))
                })
    }

    pub(super) fn native_menu_on_top(&self, frame: u64) -> bool {
        self.menus_on_top_shown(frame) && (self.menu_top.passthrough || self.menu_top.drawn.is_some_and(|(_, index, _, _)| index >= MENU_SCREENS.len()))
    }

    /// Frontend top-screen pixels (400 across), converted to native touch.
    pub fn menus_top_touch(&mut self, frame: u64, x: f32, y: f32, pressed: bool) -> Option<(f32, f32)> {
        if !self.menus_on_top_shown(frame) || self.menu_top.blocked { self.menu_top.grab = None; return None; }
        let point = [x * self.widen, y];
        if !pressed { self.menu_top.grab = None; }
        if self.menu_top.pointer != Some(point) {
            self.menu_top.pointer = Some(point);
            self.menu_top.candidates = self.menu_top.parts.iter().enumerate().rev()
                .filter(|(_, part)| !part.top && (!self.menu_top.idol_dialog || part.idol == Some(IdolGroup::Dialog)) && (part.hit(point)
                    || (inside(point, [0.0, 0.0, 400.0 * self.widen, 240.0]) && part.model.and_then(button_hash).is_some())))
                .map(|(index, part)| (index, mapped(point, part.place.to, part.place.from))).collect();
            self.menu_top.candidate = 0;
        }
        let over = self.menu_top.candidates.get(self.menu_top.candidate).map(|&(index, _)| index);
        let index = self.menu_top.grab.filter(|_| pressed).or(over)?;
        let part = &self.menu_top.parts[index];
        let validated = part.model.and_then(button_hash).is_none_or(|expected|
            self.menu_part.is_some_and(|found| found.hash == expected)
                && self.menu_looked.is_some_and(|(point, _)| Some(point) == self.pointer));
        self.menu_top.grab = (pressed && (self.menu_top.grab.is_some() || validated)).then_some(index);
        let native = mapped(point, part.place.to, part.place.from);
        Some(if part.model.and_then(button_hash).is_some() { native } else { touch_point(native) })
    }

    /// The corresponding top-screen point, useful to drive the same
    /// frontend mapping in the headless harness.
    pub fn menus_top_point(&self, frame: u64, x: f32, y: f32, button: Option<u32>) -> Option<(f32, f32)> {
        if !self.menus_on_top_shown(frame) { return None; }
        let part = self.menu_top.parts.iter().rev().find(|part| !part.top && (!self.menu_top.idol_dialog || part.idol == Some(IdolGroup::Dialog)) && (inside([x, y], part.place.from) || button.is_some()) && button.is_none_or(|button| part.model.and_then(button_hash) == Some(button)))?;
        let (x, y) = mapped([x, y], part.place.from, part.place.to);
        Some((x / self.widen, y))
    }

    pub fn hovered_menu_part(&self) -> Option<u32> { self.menu_part.map(|part| part.hash) }

    /// Host-only pointers and recorded drawings must never survive loading
    /// another console state. Enhancement settings stay with the host.
    pub(crate) fn reset_menu_after_load(&mut self) {
        self.cancel_menu_touch();
        self.menu_top = MenuTop { blocked: self.menu_top.blocked, ..MenuTop::default() };
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
        self.top_moved.clear();
        self.pause_drawn = None;
        self.cauldron_drawn = None;
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
    if mouse.menu_top.hit_frame == Some(frame) || !mouse.mouse_mode || mouse.menu_top.grab.is_some()
        || mouse.menu_click || !mouse.menus_on_top_shown(frame) { return; }
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
    // The dense native lists exhaust the game's drawing buffer when replayed
    // for both stereo eyes. Keep their native rendering until that buffer
    // can be expanded safely.
    if !mouse.menus_on_top || system.config.slider_3d > 0.0 || mouse.cauldron_shown(system.frames) { return None; }
    let bottom = mouse.bottom_scene?;
    let active = mouse.menu_screen.filter(|&(_, _, seen)| system.frames >= seen && system.frames <= seen + 3 + mouse.menu_top.duration);
    let last = mouse.menu_screen;
    let rendering = mouse.menu_top.active;
    let current = read_word(system, top + 0x40);
    if read_word(system, bottom) != BOTTOM_SCENE || current == 0 || read_word(system, bottom + 0x40) != current { return None; }
    if let Some((index, object, _, native)) = rendering.filter(|&(_, _, scene, _)| scene == current) { return Some((index, object, bottom, native)); }
    if let Some((index, object, _)) = active { return Some((index, object, bottom, false)); }
    // These child pages use native touches, rather than an A/outline
    // adapter. Keep their sliders, steps and viewer controls accessible.
    let layers = read_word(system, current + SCENE_LAYERS);
    for layer in BOTTOM_LAYERS {
        let list = read_word(system, layers + layer * LAYER_SIZE + 4);
        if list == 0 { continue; }
        let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
        if end < start || end - start > 0x1000 { continue; }
        for at in (start..end).step_by(4) {
            let model = read_word(system, at);
            let name = model_name(system, model);
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
                return Some((index, object, bottom, true));
            }
            return Some((MENU_SCREENS.len() + kind, current, bottom, true));
        }
    }
    None
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
    log::debug!("menu begin index {index} idol dialog {idol_dialog}");
    let mouse = system.aim_mouse.as_mut().unwrap();
    if mouse.saved.is_some() { return false; }
    if mouse.menu_top.drawn.is_some_and(|(_, old, owner, widen)| old != index || owner != object || widen != mouse.widen) {
        mouse.cancel_menu_touch();
        mouse.menu_looked = None;
        mouse.menu_part = None;
        mouse.menu_top.pointer = None;
        mouse.menu_top.candidates.clear();
    }
    let (native, parts) = layout_with_dialog(index, 400.0 * mouse.widen, idol_dialog);
    mouse.menu_top.idol_dialog = idol_dialog;
    mouse.menu_top.active = Some((index, object, current, native_touch));
    mouse.menu_top.passthrough = native_touch;
    mouse.menu_top.started = system.frames;
    mouse.menu_top.parts = parts;
    let parts = mouse.menu_top.parts.clone();
    let order = draw_order(system, bottom, &parts);
    system.aim_mouse.as_mut().unwrap().menu_top.draw_order = order;
    let mut moved = Vec::new();
    place_scene(system, top, top, native, true, false, &mut moved);
    if index == 12 { filter_models(system, top, true, false, &mut moved, |name| name != "MenCmnUpHeart0000"); }
    if index == 1 { chapter_models(system, top, true, false, &mut moved); }
    let mouse = system.aim_mouse.as_mut().unwrap();
    mouse.menu_top.native = moved;
    mouse.menu_top.clip = Some((native, true));
    set_clip(system, Drawing::MenuClip(START), Vec::new());
    true
}

fn place_scene(system: &mut System, top: u32, drawing: u32, place: CauldronPlace, stereo: bool, isolated: bool, moved: &mut Vec<(u32, Vec<u8>)>) {
    let scale = [place.to[2] / place.from[2], place.to[3] / place.from[3]];
    let size = [if stereo { 400.0 } else { 320.0 }, 240.0];
    let full = if isolated { place.to } else { [place.to[0] - place.from[0] * scale[0], place.to[1] - place.from[1] * scale[1], size[0] * scale[0], size[1] * scale[1]] };
    let viewport = viewport_bytes(system, top, full);
    change(system, moved, drawing + 0x44, &viewport);
    let scene = read_word(system, drawing + 0x40);
    let screen = read_word(system, drawing + 0x58);
    let camera = read_word(system, scene + SCENE_CAMERAS + screen * 4);
    if let Some((matrix, part)) = cropped_projection(system, camera, if isolated { place.from } else { [0.0, 0.0, size[0], size[1]] }, size, stereo) {
        moved.push((camera + 0x6C, matrix.to_vec()));
        system.memory.write_bytes(camera + 0x6C, &part);
    }
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

// Chapter scenery and information share a model. Select mesh nodes so
// the scenery can fill the screen while the complete information shrinks.
fn chapter_models(system: &mut System, drawing: u32, top: bool, details: bool, moved: &mut Vec<(u32, Vec<u8>)>) {
    let scene = read_word(system, drawing + 0x40);
    let layers = read_word(system, scene + SCENE_LAYERS);
    for layer in if top { 0..16 } else { BOTTOM_LAYERS } {
        let list = read_word(system, layers + layer * LAYER_SIZE + 4);
        if list == 0 { continue; }
        let (start, end) = (read_word(system, list + 0x18), read_word(system, list + 0x1C));
        if end < start || end - start > 0x1000 { continue; }
        for at in (start..end).step_by(4) {
            let model = read_word(system, at);
            let name = model_name(system, model);
            let selected = if top { name.starts_with("MenStageselectUp") } else {
                matches!(name.as_str(), "MenStageselectBt0000" | "MenStageselectBt0002" | "MenStageselectBt0003" | "MenStageselectBt0004" | "MenStageselectBt9999")
            };
            if selected && (details || !top) { foreground_materials(system, model, moved); }
            show_nodes(system, moved, model, |node| selected && if top {
                let background = name == "MenStageselectUp0000" && node != "PN_U0002";
                background != details
            } else { name != "MenStageselectBt0000" || node != "NN_B0000" });
        }
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
    let own = text_clip || matches!(drawing(system), Some(Drawing::MenuClip(_)))
        || matches!(drawing(system), Some(Drawing::MenuTop(step)) if step % 8 == 0);
    let at = system.cpu.regs[2];
    let mut old = [0; 16];
    system.memory.read_bytes(at, &mut old);
    // An isolated hub model can animate outside its nominal hit box.
    // Only grouped lists/boards need their native scrolling clips.
    let isolated = if text_clip { !mouse.menu_top.drawing_text && mouse.menu_top.text_parent.as_ref().is_some_and(|saved| !matches!(saved.drawing, Drawing::MenuClip(_))) } else if mouse.menu_top.drawing_text { false } else { match drawing(system) {
        Some(Drawing::MenuTop(step)) => mouse.menu_top.part(step / 8).is_some_and(|part| part.model.is_some()),
        _ => false,
    } };
    let mut rect = if isolated { [0.0, 0.0, 400.0 * widen, 240.0] } else { place.to };
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
        && system.aim_mouse.as_ref().is_some_and(|mouse| mouse.menu_top.active.is_some_and(|(index, _, _, _)| index == 1)
            && !mouse.menu_top.native.is_empty()) {
        system.cpu.regs[15] = TOP_TEXT_DRAWN;
        return true;
    }
    // Solo's heart count is text in layer C, separate from the bag mesh.
    // Both belong to the relocated bag, rather than the native scene.
    if place == TOP_TEXT && drawing(system).is_none()
        && system.cpu.regs[1] == 0xC
        && system.aim_mouse.as_ref().is_some_and(|mouse| !mouse.menu_top.native.is_empty() && mouse.menu_screen.is_some_and(|(index, _, _)| index == 12)) {
        system.cpu.regs[15] = TOP_TEXT_DRAWN;
        return true;
    }
    // Only the text pass excludes the old hint location. The background
    // keeps its full-screen viewport and clip throughout its mesh pass.
    let native_text = system.aim_mouse.as_ref().is_some_and(|mouse| mouse.menu_top.active.is_some()
        && !mouse.menu_top.native.is_empty() && mouse.saved.is_none());
    if native_text {
        if let Some((index, _, _, _)) = system.aim_mouse.as_ref().unwrap().menu_top.active.filter(|&(index, _, _, _)| !has_details(index)) {
            let hint_index = if system.aim_mouse.as_ref().unwrap().menu_top.idol_dialog { 22 } else { index };
            if let Some([_, y, _, height]) = hint(hint_index) {
                let width = 400.0 * system.aim_mouse.as_ref().unwrap().widen;
                let rect = if place == TOP_TEXT {
                    [0.0, if y == 0.0 { height } else { 0.0 }, width, 240.0 - height]
                } else { [0.0, 0.0, width, 240.0] };
                system.aim_mouse.as_mut().unwrap().menu_top.clip = Some((CauldronPlace {
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
        if mouse.menu_top.part(step / 8).is_some_and(|part| part.top && part.model.is_some()) {
            mouse.menu_top.drawing_text = place == TOP_TEXT;
            mouse.menu_top.text_parent = mouse.saved.take().map(Box::new);
            set_clip(system, Drawing::Viewport(place), Vec::new());
            return true;
        }
    }
    let native = matches!(drawing(system), None | Some(Drawing::Viewport(TOP_TEXT)));
    if let Some(mouse) = system.aim_mouse.as_mut() {
        mouse.menu_top.cauldron_font = place == TOP_TEXT && native && system.cpu.regs[1] == 0xB
            && mouse.menus_on_top && mouse.cauldron_on_top_shown(system.frames);
    }
    false
}

pub(super) fn font(system: &mut System) {
    let idol_group = match drawing(system) {
        Some(Drawing::MenuTop(step)) => system.aim_mouse.as_ref().and_then(|mouse| mouse.menu_top.part(step / 8)).and_then(|part| part.idol),
        _ => None,
    };
    if let Some(group) = idol_group {
        let at = system.cpu.regs[0] + 0x24;
        let (x, y) = (f32::from_bits(read_word(system, at + 8)), f32::from_bits(read_word(system, at + 12)));
        let keep = match group {
            IdolGroup::Counters => (0.0..160.0).contains(&x) && (0.0..65.0).contains(&y),
            IdolGroup::Dialog => (65.0..225.0).contains(&y),
            _ => false,
        };
        if !keep || (group == IdolGroup::Counters && system.cpu.regs[15] == 0x00A36F74) {
            system.cpu.regs[15] += 4;
            return;
        }
    }
    let heart = system.aim_mouse.as_ref().is_some_and(|mouse| mouse.menu_top.drawing_text);
    let solo = system.aim_mouse.as_ref().is_some_and(|mouse| mouse.menus_on_top
        && mouse.menu_screen.is_some_and(|(index, _, _)| index == 12));
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
    if !heart && !system.aim_mouse.as_ref().is_some_and(|mouse| mouse.menu_top.cauldron_font) { return; }
    // The native number effect draws an offset copy before its main text.
    // At the compact scale it looks like a second count; keep only the main pass.
    if heart && system.cpu.regs[15] == 0x00A36F74 {
        system.cpu.regs[15] = 0x00A36F78;
        return;
    }
    let at = system.cpu.regs[0] + 0x24;
    let mut old = [0; 16];
    system.memory.read_bytes(at, &mut old);
    let values: Vec<f32> = old.as_chunks::<4>().0.iter().map(|word| f32::from_le_bytes(*word)).collect();
    if values.iter().any(|value| !value.is_finite()) { return; }
    let values = if heart { [values[0] * 0.5, values[1] * 0.5, values[2], values[3]] } else {
        if !(0.0..30.0).contains(&values[3]) { return; }
        [values[0] * 0.7, values[1] * 0.7, 200.0 + (values[2] - 200.0) * 0.7, values[3] * 0.7]
    };
    let bytes: Vec<u8> = values.into_iter().flat_map(f32::to_le_bytes).collect();
    system.memory.write_bytes(at, &bytes);
    system.aim_mouse.as_mut().unwrap().menu_top.font_back = Some((at, old));
}

pub(super) fn font_done(system: &mut System) {
    if let Some((at, old)) = system.aim_mouse.as_mut().and_then(|mouse| mouse.menu_top.font_back.take()) {
        system.memory.write_bytes(at, &old);
    }
}

pub(super) fn bottom_translucent(system: &mut System) {
    if matches!(drawing(system), Some(Drawing::MenuTop(_)))
        && system.aim_mouse.as_ref().is_some_and(|mouse|
            mouse.menu_top.active.is_some_and(|(index, _, _, _)| matches!(index, 22 | 25) || mouse.menu_top.idol_dialog)) {
        // Idol Toss's translucent pass includes the decorative waterfall
        // and bowl glow. The opaque bowl and all labels remain separate.
        system.cpu.regs[15] = BOTTOM_TRANSLUCENT + 4;
    }
}

pub(super) fn bottom_text(system: &mut System) {
    if let Some(Drawing::MenuTop(step)) = drawing(system) {
        if system.aim_mouse.as_ref().unwrap().menu_top.part(step / 8).is_some_and(|part| !part.top && (part.model.is_some() || part.idol.is_some_and(|group| !matches!(group, IdolGroup::Counters | IdolGroup::Dialog)))) {
            // Hub labels are textures in their button meshes. Unrelated
            // text in the other groups must not be replayed with them.
            system.cpu.regs[15] = BOTTOM_TEXT + 4;
        }
    }
}

fn target(system: &mut System, eye: u32, step: usize, mut moved: Vec<(u32, Vec<u8>)>) {
    let manager = read_word(system, VSYNC_MANAGER);
    change(system, &mut moved, manager + 0x210, &eye.to_le_bytes());
    call_here(system, 0x00916588, manager, eye, Drawing::MenuTop(step), moved);
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
    if part.top && part.model.is_some() {
        // A centered guard band extends the camera's clip planes without
        // changing its screen transform or the font renderer's alignment.
        let p = part.place;
        let scale = [p.to[2] / p.from[2], p.to[3] / p.from[3]];
        let from = [-200.0, -120.0, 800.0, 480.0];
        let to = [p.to[0] + (from[0] - p.from[0]) * scale[0], p.to[1] + (from[1] - p.from[1]) * scale[1], from[2] * scale[0], from[3] * scale[1]];
        place_scene(system, top, drawing, CauldronPlace { from, to }, true, true, &mut moved);
    } else if part.model.is_some() {
        let widen = system.aim_mouse.as_ref().unwrap().widen;
        let canvas = [400.0 * widen, 240.0];
        let scale = [part.place.to[2] / part.place.from[2], part.place.to[3] / part.place.from[3]];
        let source = [part.place.from[0] - part.place.to[0] / scale[0],
            part.place.from[1] - part.place.to[1] / scale[1], canvas[0] / scale[0], canvas[1] / scale[1]];
        // Put the transform in the projection so the viewport never cuts
        // an animated mesh at its original screen or nominal button box.
        place_scene(system, top, drawing, CauldronPlace { from: source, to: [0.0, 0.0, canvas[0], canvas[1]] }, part.top, true, &mut moved);
    } else {
        place_scene(system, top, drawing, part.place, part.top, false, &mut moved);
    }
    if part.chapter {
        chapter_models(system, drawing, part.top, true, &mut moved);
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
                let selected = part.model.is_some_and(|name| name == model_name(system, model));
                show_nodes(system, &mut moved, model, |name| selected && (part.model != Some("MenCmnUpHeart0000") || name != "capbase"));
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
                let selected = group.contains(&name);
                if selected && group != IdolGroup::Launcher { foreground_materials(system, model, &mut moved); }
                show_nodes(system, &mut moved, model, |node| selected && (name != "MenColVisionBt0005"
                    || matches!(node, "BL_B0000" | "coin" | "NN_B00010" | "NN_B0006")));
            }
        }
    } else {
        filter_models(system, drawing, part.top, !part.top, &mut moved, |name| part.model.is_none_or(|model| name == model));
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
    fn layouts_preserve_shapes_and_fit_all_window_sizes() {
        for index in 0..MENU_SCREENS.len() + 9 {
            for width in [240.0, 320.0, 400.0, 240.0 * 16.0 / 9.0, 560.0, 800.0] {
                let (native, parts) = layout(index, width);
                if !has_details(index) {
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
            for point in [(80.0, 40.0), (230.0, 150.0), (35.0, 215.0), (160.0, 225.0)] {
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
                assert!((launcher.to[0] + launcher.to[2] - (width - 8.0)).abs() < 0.001);
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
            assert_eq!(parts.iter().filter(|part| part.top && part.model.is_none()).count(), 1);
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
            let parts = layout(12, width).1;
            for &(name, from) in SOLO {
                let to = parts.iter().find(|part| part.model == Some(name)).unwrap().place.to;
                let scale = (width / 320.0).min((240.0 - 30.0 * 0.7) / 240.0) * CONTROL_SCALE;
                assert!((to[2] - from[2] * scale).abs() < 0.001);
                assert!((to[3] - from[3] * scale).abs() < 0.001);
                for (axis, limit, native) in [(0, width, 320.0), (1, 240.0, 240.0)] {
                    if from[axis] == 0.0 { assert!(to[axis].abs() < 0.001); }
                    if from[axis] + from[axis + 2] == native && !(axis == 1 && name == "MenSgHomeBt0006") {
                        assert!((to[axis] + to[axis + 2] - limit).abs() < 0.001);
                    }
                }
            }
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
        for y in 0..240 {
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
    fn cauldron_hint_font_scales_and_restores_without_shrinking_counter() {
        let mut system = System::new(crate::Config::default());
        system.memory.map(0x14000000, 0x20000000, 0x1000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        let mut mouse = AimMouse::default();
        mouse.menu_top.cauldron_font = true;
        system.aim_mouse = Some(mouse);
        system.cpu.regs[0] = 0x14000000;
        let at = 0x14000024;
        let values = [1.0_f32, 1.0, 100.0, 10.0];
        let original: Vec<u8> = values.into_iter().flat_map(f32::to_le_bytes).collect();
        system.memory.write_bytes(at, &original);
        font(&mut system);
        assert_eq!(f32::from_bits(read_word(&mut system, at)), 0.7);
        assert_eq!(f32::from_bits(read_word(&mut system, at + 8)), 130.0);
        font_done(&mut system);
        assert_eq!(f32::from_bits(read_word(&mut system, at)), 1.0);
        system.memory.write_bytes(at + 12, &60.0_f32.to_le_bytes());
        font(&mut system);
        assert_eq!(f32::from_bits(read_word(&mut system, at)), 1.0);
        assert!(system.aim_mouse.as_ref().unwrap().menu_top.font_back.is_none());
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
}

// Collision solving uses a shared, unscaled canvas. Each rendered button's
// translation is expressed in that canvas; the common artwork scale cancels.
struct PhysicsCall {
    return_pc: u32,
    bodies: Vec<(u32, [f32; 2])>,
    bounds: Vec<(u32, Vec<u8>)>,
}

fn collision_delta(system: &mut System, body: u32, parts: &[Part]) -> Option<([f32; 2], f32)> {
    if body == 0 || !system.memory.is_readable(body, 0xdc) { return None; }
    let part = read_word(system, body + 0x50);
    let name_at = read_word(system, part + 0x24);
    let name = c_string(system, name_at);
    let place = parts.iter().find(|part| !part.top && part.model == Some(name.as_str()))?.place;
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
            && mouse.menu_screen.is_some_and(|(index, _, _)| index == 12)) else { return; };
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
        let parts = layout(12, 400.0).1;
        if collision_delta(system, body, &parts).is_none() { return; }
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
        && mouse.menu_top.physics.is_none() && mouse.menu_screen.is_some_and(|(index, _, _)| index == 12)) else { return };
    let width = 400.0 * mouse.widen;
    let parts = layout(12, width).1;
    let first = system.cpu.regs[1];
    let Some((delta, scale)) = collision_delta(system, first, &parts) else { return };
    let mut call = PhysicsCall { return_pc, bodies: vec![(first, delta)], bounds: Vec::new() };
    if pc == 0x002F0770 {
        let second = system.cpu.regs[2];
        let Some((delta, other_scale)) = collision_delta(system, second, &parts) else { return };
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
    fn collision_space_matches_artwork_and_preserves_solver_motion() {
        let mut system = System::new(crate::Config::default());
        system.memory.map(0x14000000, 0x20000000, 0x10000, crate::memory::Permission::RW, crate::memory::MemoryState::Private);
        system.aim_mouse = Some(AimMouse { menus_on_top: true, menu_screen: Some((12, 1, 0)), ..AimMouse::default() });
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
        let parts = layout(12, 400.0).1;
        for body in [0x14000100, 0x14000300] {
            let (delta, _) = collision_delta(&mut system, body, &parts).unwrap();
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
