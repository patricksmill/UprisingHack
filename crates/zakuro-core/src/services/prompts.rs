//! the symbols the console's font keeps among its characters, U+E000 on,
//! for its buttons and a few other things. titles write them into their
//! text, "\u{E000} OK", to show which button does what. the stand-in font
//! draws them here, each button as the input the player is using shows it:
//! the console's own look, a key, a mouse button or a controller's button.

use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};

/// a glyph's cell, as the font has it.
pub(super) const SIZE: usize = 24;
/// supersamples a side of each pixel, for smooth edges.
const SAMPLES: usize = 4;
/// how far a button's symbol moves the pen on, its whole cell, as the
/// console's do. it never changes with the label, a title may have laid
/// out a line before the player switches input.
pub(super) const ADVANCE: u8 = SIZE as u8;

/// a console input a symbol stands for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Input {
    A,
    B,
    X,
    Y,
    L,
    R,
    ZL,
    ZR,
    Home,
    /// the whole +Control Pad.
    DPad,
    Up,
    Down,
    Left,
    Right,
    /// up and down together, as for scrolling.
    UpDown,
    /// left and right together.
    LeftRight,
    CirclePad,
}

impl Input {
    pub const ALL: [Input; 17] = [
        Input::A,
        Input::B,
        Input::X,
        Input::Y,
        Input::L,
        Input::R,
        Input::ZL,
        Input::ZR,
        Input::Home,
        Input::DPad,
        Input::Up,
        Input::Down,
        Input::Left,
        Input::Right,
        Input::UpDown,
        Input::LeftRight,
        Input::CirclePad,
    ];

    /// the input's symbol the way the console draws it.
    pub fn console(self) -> Label {
        match self {
            Input::A => Label::Round("A".into()),
            Input::B => Label::Round("B".into()),
            Input::X => Label::Round("X".into()),
            Input::Y => Label::Round("Y".into()),
            Input::L => Label::Shoulder("L".into()),
            Input::R => Label::Shoulder("R".into()),
            Input::ZL => Label::Shoulder("ZL".into()),
            Input::ZR => Label::Shoulder("ZR".into()),
            Input::Home => Label::Home,
            Input::DPad => Label::Cross(None),
            Input::Up => Label::Cross(Some(Direction::Up)),
            Input::Down => Label::Cross(Some(Direction::Down)),
            Input::Left => Label::Cross(Some(Direction::Left)),
            Input::Right => Label::Cross(Some(Direction::Right)),
            Input::UpDown => Label::Cross(Some(Direction::UpDown)),
            Input::LeftRight => Label::Cross(Some(Direction::LeftRight)),
            Input::CirclePad => Label::Stick(String::new()),
        }
    }
}

/// which arms of a cross are lit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    UpDown,
    LeftRight,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// how a symbol looks.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Label {
    /// a round button with its name on it.
    Round(String),
    /// a shoulder or trigger, a rounded box with its name in it.
    Shoulder(String),
    /// a keyboard key with its name on its top.
    Key(String),
    /// a mouse with the button lit.
    Mouse(MouseButton),
    /// a cross of four arms, some lit.
    Cross(Option<Direction>),
    /// a stick, with a name on its cap.
    Stick(String),
    Home,
}

/// what the player plays with, which the labels are drawn for.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Host {
    /// the console's own buttons, nothing bound.
    #[default]
    Console,
    KeyboardMouse,
    Controller,
}

/// the symbol each input shows as.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Prompts {
    labels: [Label; Input::ALL.len()],
    host: Host,
    /// a title's own actions by name, such as Kid Icarus' Next Idol, and
    /// the symbol of the input each is bound to; for Dynamic Input Textures
    /// whose pictures stand for an action rather than a console button.
    actions: Vec<(String, Label)>,
}

impl Default for Prompts {
    fn default() -> Self {
        Prompts { labels: Input::ALL.map(Input::console), host: Host::Console, actions: Vec::new() }
    }
}

impl Prompts {
    pub fn host(&self) -> Host {
        self.host
    }

    pub fn set_host(&mut self, host: Host) {
        self.host = host;
    }

    /// the symbol of the input the action is bound to, names alike but for
    /// case, spaces and underscores
    pub fn action(&self, name: &str) -> Option<&Label> {
        let wanted = action_name(name);
        self.actions.iter().find(|(action, _)| *action == wanted).map(|(_, label)| label)
    }

    /// shows the action as label, or as nothing bound for none
    pub fn set_action(&mut self, name: &str, label: Option<Label>) {
        let name = action_name(name);
        self.actions.retain(|(action, _)| *action != name);
        if let Some(label) = label {
            self.actions.push((name, label));
        }
    }

    pub fn get(&self, input: Input) -> &Label {
        &self.labels[input as usize]
    }

    pub fn set(&mut self, input: Input, label: Label) {
        self.labels[input as usize] = label;
    }
}

/// an action's name as kept: letters and digits, lowercase.
fn action_name(name: &str) -> String {
    name.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

/// what a character of the console's symbols is drawn as.
enum Symbol {
    Input(Input),
    /// text the host's font has, as a character.
    Text(&'static str),
    /// small raised letters, the French ordinals' "er", "re", "e".
    Raised(&'static str),
}

/// the console's symbols the stand-in draws, by character.
const SYMBOLS: [(u16, Symbol); 30] = [
    (0xE000, Symbol::Input(Input::A)),
    (0xE001, Symbol::Input(Input::B)),
    (0xE002, Symbol::Input(Input::X)),
    (0xE003, Symbol::Input(Input::Y)),
    (0xE004, Symbol::Input(Input::L)),
    (0xE005, Symbol::Input(Input::R)),
    (0xE006, Symbol::Input(Input::DPad)),
    (0xE015, Symbol::Text("♠")),
    (0xE016, Symbol::Text("♦")),
    (0xE017, Symbol::Text("♥")),
    (0xE018, Symbol::Text("♣")),
    (0xE019, Symbol::Text("→")),
    (0xE01A, Symbol::Text("←")),
    (0xE01B, Symbol::Text("↑")),
    (0xE01C, Symbol::Text("↓")),
    (0xE052, Symbol::Input(Input::L)),
    (0xE053, Symbol::Input(Input::R)),
    (0xE054, Symbol::Input(Input::ZL)),
    (0xE055, Symbol::Input(Input::ZR)),
    (0xE068, Symbol::Raised("er")),
    (0xE069, Symbol::Raised("re")),
    (0xE06A, Symbol::Raised("e")),
    (0xE073, Symbol::Input(Input::Home)),
    (0xE077, Symbol::Input(Input::CirclePad)),
    (0xE079, Symbol::Input(Input::Up)),
    (0xE07A, Symbol::Input(Input::Down)),
    (0xE07B, Symbol::Input(Input::Left)),
    (0xE07C, Symbol::Input(Input::Right)),
    (0xE07D, Symbol::Input(Input::UpDown)),
    (0xE07E, Symbol::Input(Input::LeftRight)),
];

/// the characters with symbols, in order.
pub(super) fn codes() -> impl Iterator<Item = u16> {
    SYMBOLS.iter().map(|&(code, _)| code)
}

/// a symbol's cell, rows of SIZE alpha from 0 to 15, and the columns it
/// draws in.
pub(super) struct Drawn {
    pub alpha: Box<[u8]>,
    pub left: i8,
    pub width: u8,
}

/// draws the symbol for character code, with font for its text, or the
/// bitmap font where the host has none.
pub(super) fn draw(code: u16, prompts: &Prompts, font: Option<&FontRef>) -> Drawn {
    let mut canvas = Canvas::default();
    let text = Text { font };
    match SYMBOLS.iter().find(|(symbol, _)| *symbol == code).map(|(_, symbol)| symbol) {
        Some(Symbol::Input(input)) => draw_label(&mut canvas, &text, prompts.get(*input)),
        Some(Symbol::Text(characters)) => text.draw(&mut canvas, characters, Area::new(0.0, 0.0, 24.0, 24.0), 24.0, Ink::Lit),
        Some(Symbol::Raised(characters)) => text.draw(&mut canvas, characters, Area::new(0.0, 1.0, 14.0, 10.0), 12.0, Ink::Lit),
        None => {}
    }
    canvas.finish()
}

/// a label drawn size pixels square, as alpha from 0 to 255, rows from the
/// top, with font for its text, or the bitmap font where the host has none.
pub fn render(label: &Label, size: usize, font: Option<&FontRef>) -> Vec<u8> {
    let mut canvas = Canvas::sized(size.max(1));
    draw_label(&mut canvas, &Text { font }, label);
    canvas.coverage.iter().map(|&coverage| (coverage * 255.0).round() as u8).collect()
}

/// text alone, as big as fits a cell size pixels square, as alpha from 0 to
/// 255, rows from the top: a key's name without its cap.
pub fn render_text(characters: &str, size: usize, font: Option<&FontRef>) -> Vec<u8> {
    let mut canvas = Canvas::sized(size.max(1));
    Text { font }.draw(&mut canvas, characters, Area::new(1.5, 1.5, 21.0, 21.0), 24.0, Ink::Lit);
    canvas.coverage.iter().map(|&coverage| (coverage * 255.0).round() as u8).collect()
}

fn draw_label(canvas: &mut Canvas, text: &Text, label: &Label) {
    let ring = 1.6;
    match label {
        Label::Round(name) => {
            canvas.ring(|x, y| circle(x, y, 12.0, 12.0, 11.0), |x, y| circle(x, y, 12.0, 12.0, 11.0 - ring));
            text.draw(canvas, name, Area::new(5.5, 5.5, 13.0, 13.0), 14.0, Ink::Lit);
        }
        Label::Shoulder(name) => {
            canvas.ring(|x, y| rounded(x, y, 1.0, 3.0, 23.0, 21.0, 4.0), |x, y| rounded(x, y, 1.0 + ring, 3.0 + ring, 23.0 - ring, 21.0 - ring, 4.0 - ring));
            text.draw(canvas, name, Area::new(3.5, 5.5, 17.0, 13.0), 14.0, Ink::Lit);
        }
        Label::Key(name) => {
            // a lit keycap with its name cut out, with a lip below it
            canvas.fill(|x, y| rounded(x, y, 1.0, 2.0, 23.0, 22.0, 3.5), Ink::Lit);
            canvas.fill(|x, y| (2.5..21.5).contains(&x) && (18.5..20.0).contains(&y), Ink::Erased);
            text.draw(canvas, name, Area::new(2.5, 3.5, 19.0, 14.0), 14.0, Ink::Erased);
        }
        Label::Mouse(button) => {
            let body = |x: f32, y: f32| rounded(x, y, 5.0, 1.0, 19.0, 23.0, 6.5);
            canvas.ring(body, |x, y| rounded(x, y, 5.0 + ring, 1.0 + ring, 19.0 - ring, 23.0 - ring, 6.5 - ring));
            canvas.fill(|x, y| body(x, y) && (10.0..11.2).contains(&y), Ink::Lit);
            let lit = |x: f32, y: f32| match button {
                MouseButton::Left => x < 11.4,
                MouseButton::Right => x > 12.6,
                MouseButton::Middle => (10.6..13.4).contains(&x) && (3.0..9.0).contains(&y),
            };
            canvas.fill(|x, y| body(x, y) && y < 10.0 && lit(x, y), Ink::Lit);
            if *button != MouseButton::Middle {
                canvas.fill(|x, y| body(x, y) && y < 10.0 && (11.4..12.6).contains(&x), Ink::Lit);
            }
        }
        Label::Cross(direction) => {
            let cross = |x: f32, y: f32| ((8.0..16.0).contains(&x) && (1.0..23.0).contains(&y)) || ((1.0..23.0).contains(&x) && (8.0..16.0).contains(&y));
            let inner = |x: f32, y: f32| {
                ((8.0 + ring..16.0 - ring).contains(&x) && (1.0 + ring..23.0 - ring).contains(&y))
                    || ((1.0 + ring..23.0 - ring).contains(&x) && (8.0 + ring..16.0 - ring).contains(&y))
            };
            canvas.ring(cross, inner);
            let (up, down, left, right) = match direction {
                None => (false, false, false, false),
                Some(Direction::Up) => (true, false, false, false),
                Some(Direction::Down) => (false, true, false, false),
                Some(Direction::Left) => (false, false, true, false),
                Some(Direction::Right) => (false, false, false, true),
                Some(Direction::UpDown) => (true, true, false, false),
                Some(Direction::LeftRight) => (false, false, true, true),
            };
            canvas.fill(
                |x, y| {
                    let vertical = (8.0..16.0).contains(&x);
                    let horizontal = (8.0..16.0).contains(&y);
                    (up && vertical && (1.0..8.0).contains(&y))
                        || (down && vertical && (16.0..23.0).contains(&y))
                        || (left && horizontal && (1.0..8.0).contains(&x))
                        || (right && horizontal && (16.0..23.0).contains(&x))
                },
                Ink::Lit,
            );
        }
        Label::Stick(name) => {
            canvas.ring(|x, y| circle(x, y, 12.0, 12.0, 11.0), |x, y| circle(x, y, 12.0, 12.0, 11.0 - ring));
            canvas.fill(|x, y| circle(x, y, 12.0, 12.0, 7.5), Ink::Lit);
            text.draw(canvas, name, Area::new(7.0, 7.0, 10.0, 10.0), 11.0, Ink::Erased);
        }
        Label::Home => {
            // a roof over a house with a door, as the console draws it
            canvas.fill(|x, y| (3.0..=12.0).contains(&y) && (x - 12.0).abs() <= (y - 3.0) * 1.1, Ink::Lit);
            canvas.fill(|x, y| (5.0..19.0).contains(&x) && (11.0..21.0).contains(&y), Ink::Lit);
            canvas.fill(|x, y| (10.0..14.0).contains(&x) && (15.0..21.0).contains(&y), Ink::Erased);
        }
    }
}

fn circle(x: f32, y: f32, cx: f32, cy: f32, radius: f32) -> bool {
    (x - cx).powi(2) + (y - cy).powi(2) <= radius * radius
}

/// inside a box from (left, top) to (right, bottom) with rounded corners.
fn rounded(x: f32, y: f32, left: f32, top: f32, right: f32, bottom: f32, radius: f32) -> bool {
    let radius = radius.max(0.0);
    let nearest_x = x.clamp(left + radius, right - radius);
    let nearest_y = y.clamp(top + radius, bottom - radius);
    (left..right).contains(&x) && (top..bottom).contains(&y) && (x - nearest_x).powi(2) + (y - nearest_y).powi(2) <= radius * radius
}

#[derive(Clone, Copy, PartialEq)]
enum Ink {
    Lit,
    /// clears what is under it, text cut out of a lit shape.
    Erased,
}

/// a box text fits in, in pixels.
#[derive(Clone, Copy)]
struct Area {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Area {
    fn new(x: f32, y: f32, width: f32, height: f32) -> Area {
        Area { x, y, width, height }
    }
}

/// coverage of each pixel of a cell, 0 to 1. shapes are given in the
/// cell's own SIZE units, scale pixels each.
struct Canvas {
    coverage: Vec<f32>,
    size: usize,
    scale: f32,
}

impl Default for Canvas {
    fn default() -> Self {
        Canvas::sized(SIZE)
    }
}

impl Canvas {
    /// a cell size pixels square.
    fn sized(size: usize) -> Canvas {
        Canvas { coverage: vec![0.0; size * size], size, scale: size as f32 / SIZE as f32 }
    }

    /// lights or clears each pixel as much of it is inside.
    fn fill(&mut self, inside: impl Fn(f32, f32) -> bool, ink: Ink) {
        let scale = self.scale;
        for y in 0..self.size {
            for x in 0..self.size {
                let mut hits = 0;
                for sy in 0..SAMPLES {
                    for sx in 0..SAMPLES {
                        let at = |pixel: usize, sample: usize| (pixel as f32 + (sample as f32 + 0.5) / SAMPLES as f32) / scale;
                        hits += usize::from(inside(at(x, sx), at(y, sy)));
                    }
                }
                self.put(x, y, hits as f32 / (SAMPLES * SAMPLES) as f32, ink);
            }
        }
    }

    /// the band inside outer and outside inner.
    fn ring(&mut self, outer: impl Fn(f32, f32) -> bool, inner: impl Fn(f32, f32) -> bool) {
        self.fill(|x, y| outer(x, y) && !inner(x, y), Ink::Lit);
    }

    fn put(&mut self, x: usize, y: usize, amount: f32, ink: Ink) {
        let pixel = &mut self.coverage[y * self.size + x];
        *pixel = match ink {
            Ink::Lit => (*pixel + amount).min(1.0),
            Ink::Erased => *pixel * (1.0 - amount),
        };
    }

    /// the cell as 4-bit alpha, and the columns something is drawn in.
    fn finish(self) -> Drawn {
        let alpha: Box<[u8]> = self.coverage.iter().map(|&coverage| (coverage * 15.0).round() as u8).collect();
        let lit = |x: usize| (0..SIZE).any(|y| alpha[y * SIZE + x] != 0);
        match ((0..SIZE).find(|&x| lit(x)), (0..SIZE).rfind(|&x| lit(x))) {
            (Some(first), Some(last)) => Drawn { left: first as i8, width: (last + 1 - first) as u8, alpha },
            _ => Drawn { alpha, left: 0, width: 0 },
        }
    }
}

/// draws text, with the host's font or the bitmap one.
struct Text<'a> {
    font: Option<&'a FontRef<'a>>,
}

impl Text<'_> {
    /// characters centred in area, at size pixels or smaller to fit.
    fn draw(&self, canvas: &mut Canvas, characters: &str, area: Area, size: f32, ink: Ink) {
        if characters.is_empty() {
            return;
        }
        match self.font {
            Some(font) => draw_outlines(canvas, font, characters, area, size, ink),
            None => draw_bitmap(canvas, characters, area, ink),
        }
    }
}

fn draw_outlines(canvas: &mut Canvas, font: &FontRef, characters: &str, area: Area, size: f32, ink: Ink) {
    let (cell, scale) = (canvas.size, canvas.scale);
    let area = Area::new(area.x * scale, area.y * scale, area.width * scale, area.height * scale);
    let size = size * scale;
    let measure = |size: f32| {
        let scaled = font.as_scaled(PxScale::from(size));
        let width: f32 = characters.chars().map(|character| scaled.h_advance(font.glyph_id(character))).sum();
        (width, scaled.ascent() - scaled.descent())
    };
    let (width, height) = measure(size);
    let size = size * (area.width / width).min(area.height / height).min(1.0);
    let scaled = font.as_scaled(PxScale::from(size));
    let (width, _) = measure(size);
    // capitals and digits centred on their own height, not the font's
    let cap = scaled.ascent() * 0.72;
    let baseline = area.y + (area.height + cap) / 2.0;
    let mut pen = area.x + (area.width - width) / 2.0;
    let mut coverage = vec![0.0f32; cell * cell];
    for character in characters.chars() {
        let id = font.glyph_id(character);
        if let Some(outline) = font.outline_glyph(id.with_scale_and_position(size, point(pen, baseline))) {
            let bounds = outline.px_bounds();
            outline.draw(|x, y, amount| {
                let (x, y) = (bounds.min.x as i32 + x as i32, bounds.min.y as i32 + y as i32);
                if (0..cell as i32).contains(&x) && (0..cell as i32).contains(&y) {
                    let pixel = &mut coverage[y as usize * cell + x as usize];
                    *pixel = (*pixel + amount).min(1.0);
                }
            });
        }
        pen += scaled.h_advance(id);
    }
    for (index, &amount) in coverage.iter().enumerate() {
        canvas.put(index % cell, index / cell, amount, ink);
    }
}

/// the bitmap font's 8x8 characters, scaled to fit, those it lacks left out.
fn draw_bitmap(canvas: &mut Canvas, characters: &str, area: Area, ink: Ink) {
    use crate::services::glyphs::GLYPHS;

    let rows: Vec<&[u8; 8]> = characters
        .chars()
        .filter_map(|character| GLYPHS.get((character as usize).wrapping_sub(0x20)))
        .collect();
    if rows.is_empty() {
        return;
    }
    let scale = (area.width / (rows.len() * 8) as f32).min(area.height / 8.0).min(2.0);
    let left = area.x + (area.width - rows.len() as f32 * 8.0 * scale) / 2.0;
    let top = area.y + (area.height - 8.0 * scale) / 2.0;
    canvas.fill(
        |x, y| {
            let (column, row) = (((x - left) / scale).floor(), ((y - top) / scale).floor());
            if column < 0.0 || row < 0.0 || row >= 8.0 {
                return false;
            }
            let (column, row) = (column as usize, row as usize);
            rows.get(column / 8).is_some_and(|bits| bits[row] >> (7 - column % 8) & 1 != 0)
        },
        ink,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(drawn: &Drawn, x: usize, y: usize) -> bool {
        drawn.alpha[y * SIZE + x] >= 8
    }

    /// each button's symbol follows the label it is given, so switching to a
    /// key changes what the A button's character shows, and its look on the
    /// console's comes back after.
    #[test]
    fn a_symbol_follows_its_label() {
        let mut prompts = Prompts::default();
        let round = draw(0xE000, &prompts, None);
        prompts.set(Input::A, Label::Key("X".into()));
        let key = draw(0xE000, &prompts, None);
        // a ring is empty in its middle, a keycap is lit at its edge
        assert!(lit(&round, 12, 1) && !lit(&round, 3, 12));
        assert!(lit(&key, 3, 12), "the keycap's side");
        assert_ne!(round.alpha, key.alpha);
        prompts.set(Input::A, Input::A.console());
        assert_eq!(draw(0xE000, &prompts, None).alpha, round.alpha);
        // the B button's character did not follow A's label
        assert_eq!(draw(0xE001, &prompts, None).alpha, draw(0xE001, &Prompts::default(), None).alpha);
    }

    /// a pressed mouse button is the half of the mouse that is lit.
    #[test]
    fn the_mouse_lights_its_button() {
        let mut prompts = Prompts::default();
        prompts.set(Input::L, Label::Mouse(MouseButton::Left));
        let left = draw(0xE004, &prompts, None);
        assert!(lit(&left, 8, 6) && !lit(&left, 15, 6));
        prompts.set(Input::L, Label::Mouse(MouseButton::Right));
        let right = draw(0xE004, &prompts, None);
        assert!(!lit(&right, 8, 6) && lit(&right, 15, 6));
    }

    /// the +Control Pad's characters light the arms they name.
    #[test]
    fn the_cross_lights_its_arms() {
        let prompts = Prompts::default();
        let arm = |code, x, y| lit(&draw(code, &prompts, None), x, y);
        assert!(arm(0xE079, 12, 4) && !arm(0xE079, 12, 20), "up");
        assert!(arm(0xE07A, 12, 20) && !arm(0xE07A, 12, 4), "down");
        assert!(arm(0xE07D, 12, 4) && arm(0xE07D, 12, 20) && !arm(0xE07D, 4, 12), "up and down");
        assert!(!arm(0xE006, 12, 4), "the whole pad, none lit");
    }

    /// every character is listed once, in order, as the font's maps need.
    #[test]
    fn the_codes_are_in_order() {
        let codes: Vec<u16> = codes().collect();
        assert!(codes.windows(2).all(|pair| pair[0] < pair[1]), "{codes:X?}");
        assert!(codes.contains(&0xE000) && codes.contains(&0xE07E));
    }
}
