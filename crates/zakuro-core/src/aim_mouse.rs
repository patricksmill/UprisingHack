//! mouse aiming and a smaller HUD for Kid Icarus: Uprising (USA), a test of
//! what a port's overrides will do: the mouse's movement goes straight into
//! the aim delta the game's touch handling produces, with none of its
//! smoothing or flick, and the top screen's HUD and reticle are drawn
//! smaller. only the interpreter stops where this looks, so the functions
//! these places are in run in it, see FUNCTIONS.

use crate::System;

/// Kid Icarus: Uprising, the USA cartridge.
pub const PROGRAM_ID: u64 = 0x0004_0000_0003_0100;

/// the places below, which the interpreter has to run for it to stop at
/// them: every function whose code takes one in runs in it, the rest of the
/// game stays recompiled. one place a function does, the pause menu's top
/// places are in the top pass TOP_UI_AFTER_HUD is in, and the dialogs'
/// in one function, their bottom sites in another.
pub const PLACES: [u32; 11] = [
    AFTER_AIM_DELTA,
    BEFORE_CAMERA_TURN,
    SPRITE_FLUSH,
    RETICLE_DRAW,
    TOP_UI_AFTER_HUD,
    PAUSE_MENU[0].site,
    PAUSE_MENU[1].site,
    PAUSE_MENU[1].place,
    MIRRORED[0].site,
    MIRRORED[1].site,
    MIRRORED[2].site,
];

/// where 0x006D269C goes on after 0x00B12218 summed the aim delta, which it
/// left at sp+8 as reticle pixels a second, with the aim object in r4 and
/// the time the step covers in s16.
const AFTER_AIM_DELTA: u32 = 0x006D26C4;

/// where 0x006D2D8C calls 0x006D2ABC, with the box the reticle moves in
/// before the camera turns loaded in s0 to s3 (right, left, bottom, top
/// edges), the aim object in r4. on the ground the camera turns by how far
/// the reticle goes past the box.
const BEFORE_CAMERA_TURN: u32 = 0x006D2ED4;

/// the 2D sprite batch's flush, entered with the batch in r0: it draws the
/// quads added since the last one, which end at [batch+0x38], count
/// [batch+0x3C] - [batch+0x40], 0x60 bytes each, four corners of 0x18 bytes
/// with x and y first, in pixels of the screen drawn to.
const SPRITE_FLUSH: u32 = 0x0092F408;

/// in the top screen's battle UI pass, 0x0060D39C, just after it drew the
/// health bar: the narration is drawn there too.
const TOP_UI_AFTER_HUD: u32 = 0x0060D60C;

/// a bottom screen call the top screen makes too, in the same frame's top
/// pass of the 2D context it draws through, with what the last bottom pass
/// gave it.
struct ToTop {
    /// the call's bl in the bottom pass.
    site: u32,
    /// where in the top pass it is made again, just after a call there.
    place: u32,
    /// r1 points at a 4x4 projection and r2 at a 3x4 view the call takes,
    /// which are on the bottom pass's stack, so they are kept, the
    /// projection made the top screen's.
    projection: bool,
}

/// the pause menu and the dialogs over it, Retreat's for one, in the
/// middle of the top screen.
const PAUSE_MENU: [ToTop; 15] = [
    // the bottom battle UI pass, 0x0060D93C, calling 0x00A01A1C while
    // paused: the dimming, Resume and Retreat, Options, the loot and Remove
    // Display, with the pause screen in r0, the 2D context in r1 and in r2
    // whether to show it. made again after the top pass, 0x0060D3EC, drew
    // the pause screen's label, which it does only while paused
    ToTop { site: 0x0060DBB4, place: 0x0060D68C, projection: false },
    // the dialogs' bottom pass, 0x00722A50, calling 0x00721C64, the
    // dimming and the box, with the dialogs' 2D context in r0 and how far
    // they are shown in s0, then 0x00721484, the buttons. made again after
    // their top pass, 0x007255AC, drew the corner icon it draws every frame
    ToTop { site: 0x00722AF4, place: 0x00725828, projection: false },
    ToTop { site: 0x00722B0C, place: 0x00725828, projection: false },
    // and once their 2D is done, the text, drawn through the 3D renderer
    // (r0) with a projection for the bottom screen: the lights (0x00909D54),
    // the camera (0x0090A5B4), the start (0x0090FFA0), the message and the
    // buttons' text (0x00909ACC, the text in r0), the end (0x0090A6A4).
    // made again after the top pass's 2D is done, for the right eye's
    // buffer, which 0x00916588 left it drawing into, and once it went back
    // to the left eye's, as the top pass draws its own text in both
    ToTop { site: 0x00722BA4, place: 0x007258A0, projection: false },
    ToTop { site: 0x00722C30, place: 0x007258A0, projection: true },
    ToTop { site: 0x00722C3C, place: 0x007258A0, projection: false },
    ToTop { site: 0x00722D14, place: 0x007258A0, projection: false },
    ToTop { site: 0x00722DAC, place: 0x007258A0, projection: false },
    ToTop { site: 0x00722DB8, place: 0x007258A0, projection: false },
    ToTop { site: 0x00722BA4, place: 0x007258C0, projection: false },
    ToTop { site: 0x00722C30, place: 0x007258C0, projection: true },
    ToTop { site: 0x00722C3C, place: 0x007258C0, projection: false },
    ToTop { site: 0x00722D14, place: 0x007258C0, projection: false },
    ToTop { site: 0x00722DAC, place: 0x007258C0, projection: false },
    ToTop { site: 0x00722DB8, place: 0x007258C0, projection: false },
];

/// how far right the pause menu goes on the top screen, its 320 pixels in
/// the middle of the top's 400, and so how far left a touch there goes.
const PAUSE_MENU_OFFSET: f32 = 40.0;

/// where the pause screen's sprite flushes come back to for Resume and
/// Retreat, their buttons, icons and words, one quad each a flush, which go
/// to the top screen's left and right edges.
const PAUSE_BUTTONS: [u32; 4] = [0x00A0D788, 0x00A0D88C, 0x00A0D9F8, 0x00A0DB00];

/// the rows Resume and Retreat are in, Resume from the left edge to 148,
/// Retreat from 172 to the right edge, bottom screen pixels.
const PAUSE_BUTTON_ROWS: std::ops::Range<f32> = 55.0..111.0;
const RESUME_RIGHT: f32 = 148.0;
const RETREAT_LEFT: f32 = 172.0;

/// and for Remove Display, in the bottom right corner from 193 and 204 on,
/// which goes to the top screen's.
const REMOVE_DISPLAY: [u32; 2] = [0x00A0D524, 0x00A0D5DC];
const REMOVE_DISPLAY_CORNER: [f32; 2] = [193.0, 204.0];

/// draws a message's line, shadow then text, from where the message says,
/// x at +0x14, y at +0x18, its size at +0x1C, +0x20: r0 the message, r1 the
/// 2D context.
const DRAW_MESSAGE_LINE: u32 = 0x009E9A14;

/// the top HUD's score, flushed from the 2D context the narration's message
/// is part of.
const SCORE_FLUSH_CALLER: u32 = 0x009F2F90;

/// the narration's message in the 2D context, shown when its first byte
/// and its line at +4 are set.
const NARRATION: u32 = 0x3124;

/// draws the speakers' portraits, up to four 256-pixel squares placed for
/// the bottom screen: r0 the portraits, r1 the 2D context.
const DRAW_PORTRAITS: u32 = 0x009E2BB4;

/// where in the 2D context the bottom screen's portraits are kept, a pointer.
const PORTRAITS: u32 = 0xC3C;

/// how far the bottom screen's UI has faded out, 0 when fully shown: the
/// bottom pass hands it on (0x0060DA48), and the portraits are drawn at one
/// less it, and not at all once it is 1 (0x00A182C4).
const BOTTOM_FADE: u32 = 0x2454;

/// a bottom screen element drawn on the top screen too: the call the bottom
/// pass makes to draw it, which the top pass makes again with what the
/// bottom pass gave it, and where its sprites go.
struct Mirrored {
    /// the call's bl, in the bottom screen's background drawing (0x00A17F70),
    /// with the bottom UI in r0, the 2D context in r1 and floats in s0..s2.
    site: u32,
    /// the bottom screen's corner the element is drawn by.
    from: [f32; 2],
    /// the top screen's corner it goes by, and how far down from it in HUD
    /// pixels, the height of what is above it there.
    to: [f32; 2],
    below: f32,
    /// its size against the rest of the HUD.
    size: f32,
    /// two rows swapped, bottom screen pixels: quads centred above the first
    /// go down by the second, the rest up by the third.
    swap_rows: Option<[f32; 3]>,
    /// where its sprite flushes come back to that draw something left off
    /// the top screen.
    hidden: &'static [u32],
}

/// the chapter title and hearts, under the score, with the hearts held on
/// the top row and the intensity and the chapter's hearts a little below it;
/// the item box and power icon, under the health bar. the chapter title is
/// left off.
const MIRRORED: [Mirrored; 3] = [
    Mirrored { site: 0x00A181CC, from: [320.0, 0.0], to: [400.0, 0.0], below: 40.0, size: 1.0, swap_rows: Some([24.0, 31.0, 16.0]), hidden: &[0x00A192E4] },
    Mirrored { site: 0x00A18400, from: [0.0, 194.0], to: [0.0, 0.0], below: 36.0, size: 0.75, swap_rows: None, hidden: &[] },
    Mirrored { site: 0x00A18430, from: [0.0, 194.0], to: [0.0, 0.0], below: 36.0, size: 0.75, swap_rows: None, hidden: &[] },
];

/// what a mirrored element's call was given in the bottom pass, made again
/// in the next frame's top pass.
#[derive(Clone, Copy)]
struct Recorded {
    target: u32,
    regs: [u32; 4],
    floats: [u32; 8],
    /// the projection and the view a call takes, see ToTop::projection.
    matrices: Option<[u32; 28]>,
}

/// what the game's code is drawing on the top screen for us.
#[derive(Clone, Copy, PartialEq)]
enum Drawing {
    Portraits,
    Mirrored(usize),
    Narration,
    PauseMenu(usize),
}

impl Drawing {
    /// where in the top pass it is drawn, and comes back to.
    fn place(self) -> u32 {
        match self {
            Drawing::PauseMenu(index) => PAUSE_MENU[index].place,
            _ => TOP_UI_AFTER_HUD,
        }
    }
}

/// what the CPU held where the call was made in the game's place, put back
/// when the call returns there, and the narration's position while it is
/// moved.
struct Saved {
    regs: [u32; 16],
    cpsr: zakuro_cpu::psr::Psr,
    vfp: [u32; 32],
    drawing: Drawing,
    moved: Option<(u32, [u8; 8])>,
}

/// where placing the reticle, 0x007C4854, hands each of its two parts the
/// size and place to draw it at: r1 points at them, the size first, r4
/// counts the parts. the size it keeps (reticle+0x5C) goes on into what the
/// reticle takes in, so only this copy is made smaller.
const RETICLE_DRAW: u32 = 0x007C4DC0;

pub struct AimMouse {
    /// reticle pixels moved since the game last took them.
    pub pending: [f32; 2],
    /// whether the mouse is aiming, which on the ground keeps the reticle
    /// in the middle and turns the camera by all of the mouse's movement.
    pub captured: bool,
    /// how big the top screen's HUD and reticle are drawn, 1 as the game
    /// draws them.
    pub hud_scale: f32,
    /// whether the narration is drawn on the top screen too.
    pub narration_on_top: bool,
    /// how big the portraits are drawn on the top screen, 0 for not at all.
    pub portrait_scale: f32,
    /// the 2D context the top HUD was last drawn through.
    context: Option<u32>,
    /// the CPU while the game draws the portraits or the narration on the
    /// top screen.
    saved: Option<Saved>,
    /// whether the bottom screen's hearts, item box and power icon are drawn
    /// on the top screen too.
    pub mirror_bottom: bool,
    /// the mirrored elements' calls from the last bottom pass, by MIRRORED's
    /// order.
    recorded: [Option<Recorded>; 3],
    /// whether the pause menu and the dialogs over it are drawn on the top
    /// screen too.
    pub pause_on_top: bool,
    /// their calls from the last bottom pass, by PAUSE_MENU's order.
    pause: [Option<Recorded>; 15],
    /// the frame the pause menu was last drawn on the top screen in.
    pause_drawn: Option<u64>,
    /// and the last one Resume, Retreat or Remove Display was drawn by the
    /// top screen's edges in.
    edges_drawn: Option<u64>,
}

impl Default for AimMouse {
    fn default() -> AimMouse {
        AimMouse {
            pending: [0.0; 2],
            captured: false,
            hud_scale: 1.0,
            narration_on_top: false,
            portrait_scale: 0.0,
            context: None,
            saved: None,
            mirror_bottom: false,
            recorded: [None; 3],
            pause_on_top: false,
            pause: [None; 15],
            pause_drawn: None,
            edges_drawn: None,
        }
    }
}

impl AimMouse {
    /// for the game, drawing the HUD at hud_scale, the narration on the top
    /// screen too when narration_on_top, the portraits there at
    /// portrait_scale, 0 for not, the hearts, item box and power icon there
    /// when mirror_bottom, and the pause menu when pause_on_top.
    pub fn new(hud_scale: f32, narration_on_top: bool, portrait_scale: f32, mirror_bottom: bool, pause_on_top: bool) -> AimMouse {
        AimMouse { hud_scale, narration_on_top, portrait_scale, mirror_bottom, pause_on_top, ..AimMouse::default() }
    }

    pub fn add(&mut self, dx: f32, dy: f32) {
        self.pending[0] += dx;
        self.pending[1] += dy;
    }

    /// whether the pause menu or a dialog over it was drawn on the top
    /// screen lately, by frame, the frames the System counts: at 30 FPS the
    /// game draws in every other one, and in a slow one a little less.
    pub fn pause_menu_shown(&self, frame: u64) -> bool {
        self.pause_drawn.is_some_and(|drawn| frame <= drawn + 3)
    }

    /// the bottom screen pixel a touch of the top screen's pixel x, y
    /// touches while the pause menu is shown there, frame as for
    /// pause_menu_shown. none off the bottom screen, or beside Resume,
    /// Retreat or Remove Display by the top screen's edges, where the
    /// bottom screen has them but the top screen does not.
    pub fn pause_menu_touch(&self, frame: u64, x: f32, y: f32) -> Option<(f32, f32)> {
        let edges = self.edges_drawn.is_some_and(|drawn| frame <= drawn + 3);
        let right = 2.0 * PAUSE_MENU_OFFSET;
        let x = if edges && PAUSE_BUTTON_ROWS.contains(&y) {
            if x < RESUME_RIGHT {
                x
            } else if x >= RETREAT_LEFT + right {
                x - right
            } else {
                return None;
            }
        } else if edges && y >= REMOVE_DISPLAY_CORNER[1] && x >= REMOVE_DISPLAY_CORNER[0] + PAUSE_MENU_OFFSET {
            if x < REMOVE_DISPLAY_CORNER[0] + right {
                return None;
            }
            x - right
        } else {
            x - PAUSE_MENU_OFFSET
        };
        ((0.0..320.0).contains(&x) && (0.0..240.0).contains(&y)).then_some((x, y))
    }
}

/// does what the instruction about to run calls for: the mouse's movement
/// where the game has just summed the aim delta, the camera box on the
/// ground, the HUD's and the reticle's size where they are drawn.
pub(crate) fn apply(system: &mut System, pc: u32) {
    match pc {
        AFTER_AIM_DELTA => add_delta(system),
        BEFORE_CAMERA_TURN => close_box(system),
        SPRITE_FLUSH => {
            if system.cpu.regs[14] == SCORE_FLUSH_CALLER {
                let context = system.cpu.regs[0];
                if let Some(aim_mouse) = &mut system.aim_mouse {
                    aim_mouse.context = Some(context);
                }
            }
            shrink_hud(system);
        }
        RETICLE_DRAW => shrink_reticle(system),
        TOP_UI_AFTER_HUD => top_screen_extras(system),
        _ if PAUSE_MENU.iter().any(|to_top| to_top.site == pc || to_top.place == pc) => {
            if let Some(index) = PAUSE_MENU.iter().position(|to_top| to_top.site == pc) {
                let mut recorded = recorded_call(system, pc);
                if let Some(recorded) = recorded.as_mut().filter(|_| PAUSE_MENU[index].projection) {
                    let mut bytes = [0u8; 0x70];
                    system.memory.read_bytes(system.cpu.regs[1], &mut bytes[..0x40]);
                    system.memory.read_bytes(system.cpu.regs[2], &mut bytes[0x40..]);
                    let words = bytes.as_chunks::<4>().0.iter().map(|word| u32::from_le_bytes(*word));
                    let mut matrices = [0u32; 28];
                    matrices.iter_mut().zip(words).for_each(|(matrix, word)| *matrix = word);
                    recorded.matrices = Some(matrices);
                }
                if let Some(aim_mouse) = &mut system.aim_mouse {
                    for (kept, to_top) in aim_mouse.pause.iter_mut().zip(&PAUSE_MENU) {
                        if to_top.site == pc {
                            *kept = recorded;
                        }
                    }
                }
            }
            if PAUSE_MENU.iter().any(|to_top| to_top.place == pc) {
                top_screen_pause_menu(system, pc);
            }
        }
        _ if MIRRORED.iter().any(|mirrored| mirrored.site == pc) => record(system, pc),
        _ => {}
    }
}

/// on the ground, shrinks the box to its middle point, so that any movement
/// of the reticle is past it and turns the camera.
fn close_box(system: &mut System) {
    if !system.aim_mouse.as_ref().is_some_and(|aim_mouse| aim_mouse.captured) {
        return;
    }
    let aim = system.cpu.regs[4];
    let mut flags = [0u8; 0xD6];
    system.memory.read_bytes(aim, &mut flags);
    // +0x81 is set on the ground, +0xD5 for anyone but the local player
    if flags[0x81] == 0 || flags[0xD5] != 0 {
        return;
    }
    let regs = &mut system.cpu.vfp.regs;
    let x = (f32::from_bits(regs[0]) + f32::from_bits(regs[1])) * 0.5;
    let y = (f32::from_bits(regs[2]) + f32::from_bits(regs[3])) * 0.5;
    regs[0] = x.to_bits();
    regs[1] = x.to_bits();
    regs[2] = y.to_bits();
    regs[3] = y.to_bits();
}

/// adds what the mouse moved to the local player's aim delta, divided by
/// the time the step covers since the delta is a speed.
fn add_delta(system: &mut System) {
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    let [dx, dy] = std::mem::take(&mut aim_mouse.pending);
    let aim = system.cpu.regs[4];
    let mut remote = [0u8];
    system.memory.read_bytes(aim + 0xD5, &mut remote);
    let dt = f32::from_bits(system.cpu.vfp.regs[16]);
    if remote[0] != 0 || dt <= 0.0 {
        return;
    }
    let out = system.cpu.regs[13] + 8;
    let mut delta = [0u8; 8];
    system.memory.read_bytes(out, &mut delta);
    let x = f32::from_le_bytes(delta[0..4].try_into().unwrap()) + dx / dt;
    let y = f32::from_le_bytes(delta[4..8].try_into().unwrap()) + dy / dt;
    delta[0..4].copy_from_slice(&x.to_le_bytes());
    delta[4..8].copy_from_slice(&y.to_le_bytes());
    system.memory.write_bytes(out, &delta);
}

/// draws the portraits and then the narration's line on the top screen as
/// well: the game's own drawing of them, each called here as if the pass
/// called it. back here after each, everything is put back, and once both
/// are drawn the pass goes on.
fn top_screen_extras(system: &mut System) {
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    if let Some(back) = aim_mouse.saved.take() {
        system.cpu.regs = back.regs;
        system.cpu.cpsr = back.cpsr;
        system.cpu.vfp.regs = back.vfp;
        if let Some((message, position)) = back.moved {
            system.memory.write_bytes(message + 0x14, &position);
        }
        let next = match back.drawing {
            Drawing::Portraits => 0,
            Drawing::Mirrored(index) => index + 1,
            Drawing::Narration | Drawing::PauseMenu(_) => return,
        };
        if !start_mirrored(system, next) {
            start_narration(system);
        }
        return;
    }
    if !start_portraits(system) && !start_mirrored(system, 0) {
        start_narration(system);
    }
}

/// keeps what the bottom pass gives a mirrored element's call, for the top
/// pass to make it again.
fn record(system: &mut System, site: u32) {
    let Some(index) = MIRRORED.iter().position(|mirrored| mirrored.site == site) else { return };
    let Some(recorded) = recorded_call(system, site) else { return };
    if let Some(aim_mouse) = &mut system.aim_mouse {
        aim_mouse.recorded[index] = Some(recorded);
    }
}

/// the call the bl at site is about to make, and what it is given.
fn recorded_call(system: &mut System, site: u32) -> Option<Recorded> {
    let mut word = [0u8; 4];
    system.memory.read_bytes(site, &mut word);
    let word = u32::from_le_bytes(word);
    // a bl: its target is 8 past it and the offset in words. one whose
    // condition fails makes no call, what the registers hold is not for it
    if word >> 24 & 0x0F != 0x0B || !zakuro_cpu::psr::check_condition(word >> 28, &system.cpu.cpsr) {
        return None;
    }
    let offset = ((word << 8) as i32 >> 6) as u32;
    let target = site.wrapping_add(8).wrapping_add(offset);
    let cpu = &system.cpu;
    Some(Recorded {
        target,
        regs: [cpu.regs[0], cpu.regs[1], cpu.regs[2], cpu.regs[3]],
        floats: cpu.vfp.regs[..8].try_into().unwrap(),
        matrices: None,
    })
}

/// draws the pause menu, or a dialog over it, on the top screen as well,
/// each call the last bottom pass made that comes here made again in turn,
/// everything put back after each.
fn top_screen_pause_menu(system: &mut System, place: u32) {
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    let mut next = 0;
    if let Some(back) = aim_mouse.saved.take() {
        system.cpu.regs = back.regs;
        system.cpu.cpsr = back.cpsr;
        system.cpu.vfp.regs = back.vfp;
        let Drawing::PauseMenu(index) = back.drawing else { return };
        next = index + 1;
    }
    if !aim_mouse.pause_on_top {
        return;
    }
    let Some(index) = (next..PAUSE_MENU.len()).find(|&index| PAUSE_MENU[index].place == place && aim_mouse.pause[index].is_some()) else {
        return;
    };
    let Some(recorded) = aim_mouse.pause[index].take() else { return };
    aim_mouse.pause_drawn = Some(system.frames);
    call_here(system, recorded.target, recorded.regs[0], recorded.regs[1], Drawing::PauseMenu(index), None);
    system.cpu.regs[2] = recorded.regs[2];
    system.cpu.regs[3] = recorded.regs[3];
    system.cpu.vfp.regs[..8].copy_from_slice(&recorded.floats);
    if let Some(mut matrices) = recorded.matrices {
        // the projection's second row turns the bottom screen's 320 pixels
        // into its width, the top's middle 320 of 400 is that much less
        for word in &mut matrices[4..8] {
            *word = (f32::from_bits(*word) * 320.0 / 400.0).to_bits();
        }
        // below the stack the call starts with, put back after it
        let at = system.cpu.regs[13] - 0x70;
        let bytes: Vec<u8> = matrices.iter().flat_map(|word| word.to_le_bytes()).collect();
        system.memory.write_bytes(at, &bytes);
        system.cpu.regs[1] = at;
        system.cpu.regs[2] = at + 0x40;
        system.cpu.regs[13] = at;
    }
}

/// starts drawing the first mirrored element from index on that the last
/// bottom pass drew, whether there was one. each is drawn once: the bottom
/// pass records it again when it draws it again.
fn start_mirrored(system: &mut System, from: usize) -> bool {
    let Some(aim_mouse) = &mut system.aim_mouse else { return false };
    if !aim_mouse.mirror_bottom {
        return false;
    }
    for index in from..MIRRORED.len() {
        let Some(recorded) = aim_mouse.recorded[index].take() else { continue };
        call_here(system, recorded.target, recorded.regs[0], recorded.regs[1], Drawing::Mirrored(index), None);
        system.cpu.regs[2] = recorded.regs[2];
        system.cpu.regs[3] = recorded.regs[3];
        system.cpu.vfp.regs[..8].copy_from_slice(&recorded.floats);
        return true;
    }
    false
}

/// has the game's code run function as if called from here with r0 and r1,
/// coming back here, the CPU kept to be put back then.
fn call_here(system: &mut System, function: u32, r0: u32, r1: u32, drawing: Drawing, moved: Option<(u32, [u8; 8])>) {
    let cpu = &mut system.cpu;
    let saved = Saved { regs: cpu.regs, cpsr: cpu.cpsr, vfp: cpu.vfp.regs, drawing, moved };
    cpu.regs[0] = r0;
    cpu.regs[1] = r1;
    cpu.regs[14] = drawing.place();
    cpu.regs[15] = function;
    cpu.cpsr.thumb = false;
    if let Some(aim_mouse) = &mut system.aim_mouse {
        aim_mouse.saved = Some(saved);
    }
}

/// the 2D context the top HUD was drawn through, when there is one.
fn context(system: &System) -> Option<u32> {
    system.aim_mouse.as_ref().and_then(|aim_mouse| aim_mouse.context)
}

/// starts drawing the portraits on the top screen, whether it did.
fn start_portraits(system: &mut System) -> bool {
    let scale = system.aim_mouse.as_ref().map_or(0.0, |aim_mouse| aim_mouse.portrait_scale);
    let Some(context) = context(system).filter(|_| scale > 0.0) else { return false };
    let mut pointer = [0u8; 4];
    system.memory.read_bytes(context + PORTRAITS, &mut pointer);
    let portraits = u32::from_le_bytes(pointer);
    if portraits == 0 {
        return false;
    }
    let mut fade = [0u8; 4];
    system.memory.read_bytes(context + BOTTOM_FADE, &mut fade);
    let fade = f32::from_le_bytes(fade);
    if !(fade < 1.0) {
        return false;
    }
    call_here(system, DRAW_PORTRAITS, portraits, context, Drawing::Portraits, None);
    system.cpu.vfp.regs[0] = (1.0 - fade.max(0.0)).to_bits();
    true
}

/// starts drawing the narration's line on the top screen, centred at its
/// bottom, when a line is shown.
fn start_narration(system: &mut System) {
    let on = system.aim_mouse.as_ref().is_some_and(|aim_mouse| aim_mouse.narration_on_top);
    let Some(context) = context(system).filter(|_| on) else { return };
    let message = context + NARRATION;
    let mut head = [0u8; 0x24];
    system.memory.read_bytes(message, &mut head);
    let word = |at: usize| u32::from_le_bytes(head[at..at + 4].try_into().unwrap());
    if head[0] == 0 || word(4) == 0 {
        return;
    }
    let (width, height) = (f32::from_bits(word(0x1C)), f32::from_bits(word(0x20)));
    if !(0.0..=400.0).contains(&width) || !(0.0..=240.0).contains(&height) {
        return;
    }
    let position: [u8; 8] = head[0x14..0x1C].try_into().unwrap();
    let mut moved = [0u8; 8];
    moved[..4].copy_from_slice(&((400.0 - width) * 0.5).to_le_bytes());
    moved[4..].copy_from_slice(&(240.0 - height - 8.0).to_le_bytes());
    system.memory.write_bytes(message + 0x14, &moved);
    call_here(system, DRAW_MESSAGE_LINE, message, context, Drawing::Narration, Some((message, position)));
}

/// what the game's code is drawing on the top screen for us now, if anything.
fn drawing(system: &System) -> Option<Drawing> {
    system.aim_mouse.as_ref().and_then(|aim_mouse| aim_mouse.saved.as_ref()).map(|saved| saved.drawing)
}

/// the quads a flush is about to draw, where they are and what they hold.
fn flushed_quads(system: &mut System) -> Option<(u32, Vec<u8>)> {
    let batch = system.cpu.regs[0];
    let mut header = [0u8; 12];
    system.memory.read_bytes(batch + 0x38, &mut header);
    let word = |i: usize| u32::from_le_bytes(header[i * 4..i * 4 + 4].try_into().unwrap());
    let (end, count) = (word(0), word(1).wrapping_sub(word(2)));
    if count == 0 || count > 0x400 || end < count * 0x60 {
        return None;
    }
    let mut quads = vec![0u8; (count * 0x60) as usize];
    system.memory.read_bytes(end - count * 0x60, &mut quads);
    Some((end - count * 0x60, quads))
}

fn get(quads: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(quads[at..at + 4].try_into().unwrap())
}

fn set(quads: &mut [u8], at: usize, value: f32) {
    quads[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// puts a mirrored element's sprites, drawn for the bottom screen, by its
/// corner of the top screen, at the HUD's scale, below what is there; those
/// it leaves off go far off the screen.
fn place_mirrored(system: &mut System, mirrored: &Mirrored) {
    let scale = system.aim_mouse.as_ref().map_or(1.0, |aim_mouse| aim_mouse.hud_scale);
    let hide = mirrored.hidden.contains(&system.cpu.regs[14]);
    let Some((at, mut quads)) = flushed_quads(system) else { return };
    let down = if mirrored.to[1] == 0.0 { mirrored.below * scale } else { -mirrored.below * scale };
    let scale = scale * mirrored.size;
    for quad in (0..quads.len()).step_by(0x60) {
        let middle = (0..4).map(|corner| get(&quads, quad + corner * 0x18 + 4)).sum::<f32>() / 4.0;
        let moved = match mirrored.swap_rows {
            Some([split, down, up]) => if middle < split { down } else { -up },
            None => 0.0,
        };
        for corner in (quad..quad + 0x60).step_by(0x18) {
            let (x, y) = if hide {
                (-10000.0, -10000.0)
            } else {
                (
                    mirrored.to[0] + (get(&quads, corner) - mirrored.from[0]) * scale,
                    mirrored.to[1] + down + (get(&quads, corner + 4) + moved - mirrored.from[1]) * scale,
                )
            };
            set(&mut quads, corner, x);
            set(&mut quads, corner + 4, y);
        }
    }
    system.memory.write_bytes(at, &quads);
}

/// puts the pause menu or a dialog, drawn for the bottom screen, in the
/// middle of the top screen as big as it is drawn, so a touch there is the
/// same one moved left, see pause_menu_touch. what spans the bottom screen,
/// the dimming and the band behind Options, spans the top screen, and
/// Resume, Retreat and Remove Display go by its edges as they are by the
/// bottom screen's.
fn place_pause_menu(system: &mut System) {
    let caller = system.cpu.regs[14];
    let (buttons, corner) = (PAUSE_BUTTONS.contains(&caller), REMOVE_DISPLAY.contains(&caller));
    let Some((at, mut quads)) = flushed_quads(system) else { return };
    for quad in (0..quads.len()).step_by(0x60) {
        let xs = (0..4).map(|corner| get(&quads, quad + corner * 0x18));
        let (left, right) = xs.fold((f32::MAX, f32::MIN), |(left, right), x| (left.min(x), right.max(x)));
        let across = left <= 0.5 && right >= 319.5;
        // Resume on the left, Retreat on the right
        let by_right = corner || (buttons && left + right >= 320.0);
        let offset = match (buttons || corner, by_right) {
            (false, _) => PAUSE_MENU_OFFSET,
            (true, false) => 0.0,
            (true, true) => 2.0 * PAUSE_MENU_OFFSET,
        };
        for corner in (quad..quad + 0x60).step_by(0x18) {
            let x = get(&quads, corner);
            set(&mut quads, corner, if across { x * 400.0 / 320.0 } else { x + offset });
        }
    }
    system.memory.write_bytes(at, &quads);
    if buttons || corner {
        let frame = system.frames;
        if let Some(aim_mouse) = &mut system.aim_mouse {
            aim_mouse.edges_drawn = Some(frame);
        }
    }
}

/// puts the portraits, drawn for the bottom screen, in the top screen's
/// bottom corners, smaller: a speaker on the left half of the bottom screen
/// in the bottom left corner, one on the right in the bottom right.
fn place_portraits(system: &mut System) {
    let scale = system.aim_mouse.as_ref().map_or(1.0, |aim_mouse| aim_mouse.portrait_scale);
    let Some((at, mut quads)) = flushed_quads(system) else { return };
    for quad in (0..quads.len()).step_by(0x60) {
        let middle = (0..4).map(|corner| get(&quads, quad + corner * 0x18)).sum::<f32>() / 4.0;
        if !middle.is_finite() {
            return;
        }
        // the bottom screen's edge the speaker is by, and the top screen's
        let (from, to) = if middle < 160.0 { (0.0, 0.0) } else { (320.0, 400.0) };
        for corner in (quad..quad + 0x60).step_by(0x18) {
            let x = to + (get(&quads, corner) - from) * scale;
            let y = 240.0 + (get(&quads, corner + 4) - 240.0) * scale;
            set(&mut quads, corner, x);
            set(&mut quads, corner + 4, y);
        }
    }
    system.memory.write_bytes(at, &quads);
}

/// the top screen's battle HUD, by where its sprite flushes return to:
/// score, health bar, wings, status icon. found in the USA build's air and
/// ground battles, other HUD stays the size the game draws it.
const TOP_HUD: [u32; 7] = [0x009F2F90, 0x009F45FC, 0x009F4800, 0x009F4BD4, 0x009F52C8, 0x009F557C, 0x00725828];

/// whether a sprite batch flushed from caller draws the top screen's HUD,
/// which the narration's line is part of while it is drawn there.
fn top_hud(system: &System, caller: u32) -> bool {
    TOP_HUD.contains(&caller) || drawing(system) == Some(Drawing::Narration)
}

/// draws a top screen HUD element smaller: the quads the flush is about to
/// draw, scaled together toward the edge or corner they sit by, or their
/// own middle when they are by none, like a marker on an enemy.
fn shrink_hud(system: &mut System) {
    match drawing(system) {
        Some(Drawing::Portraits) => return place_portraits(system),
        Some(Drawing::Mirrored(index)) => return place_mirrored(system, &MIRRORED[index]),
        Some(Drawing::PauseMenu(_)) => return place_pause_menu(system),
        _ => {}
    }
    let scale = system.aim_mouse.as_ref().map_or(1.0, |aim_mouse| aim_mouse.hud_scale);
    if scale == 1.0 || !top_hud(system, system.cpu.regs[14]) {
        return;
    }
    let batch = system.cpu.regs[0];
    let mut header = [0u8; 12];
    system.memory.read_bytes(batch + 0x38, &mut header);
    let word = |i: usize| u32::from_le_bytes(header[i * 4..i * 4 + 4].try_into().unwrap());
    let (end, count) = (word(0), word(1).wrapping_sub(word(2)));
    if count == 0 || count > 0x400 || end < count * 0x60 {
        return;
    }
    let mut quads = vec![0u8; (count * 0x60) as usize];
    system.memory.read_bytes(end - count * 0x60, &mut quads);
    let get = |quads: &[u8], at: usize| f32::from_le_bytes(quads[at..at + 4].try_into().unwrap());
    let corners = (0..quads.len()).step_by(0x18);
    let (mut min, mut max) = ([f32::MAX; 2], [f32::MIN; 2]);
    for at in corners.clone() {
        for axis in 0..2 {
            let value = get(&quads, at + axis * 4);
            if !value.is_finite() {
                return;
            }
            min[axis] = min[axis].min(value);
            max[axis] = max[axis].max(value);
        }
    }
    // the whole screen, a fade or a flash, stays as it is, the narration's
    // line, as wide as it may be, doesn't
    let narration = drawing(system) == Some(Drawing::Narration);
    if !narration && (max[0] - min[0] > 300.0 || max[1] - min[1] > 200.0) {
        return;
    }
    let anchor = |min: f32, max: f32, size: f32| {
        if min < size * 0.12 {
            0.0
        } else if max > size * 0.88 {
            size
        } else {
            (min + max) * 0.5
        }
    };
    // the narration's line shrinks toward the middle of the bottom edge,
    // where it is put
    let anchor = if narration { [200.0, 240.0] } else { [anchor(min[0], max[0], 400.0), anchor(min[1], max[1], 240.0)] };
    for at in corners {
        for axis in 0..2 {
            let value = get(&quads, at + axis * 4);
            let scaled = anchor[axis] + (value - anchor[axis]) * scale;
            quads[at + axis * 4..at + axis * 4 + 4].copy_from_slice(&scaled.to_le_bytes());
        }
    }
    system.memory.write_bytes(end - count * 0x60, &quads);
}

/// draws the reticle at the HUD's scale: its parts' size, made smaller once
/// before the first is drawn.
fn shrink_reticle(system: &mut System) {
    let scale = system.aim_mouse.as_ref().map_or(1.0, |aim_mouse| aim_mouse.hud_scale);
    if scale == 1.0 || system.cpu.regs[4] != 0 {
        return;
    }
    let at = system.cpu.regs[1];
    let mut size = [0u8; 4];
    system.memory.read_bytes(at, &mut size);
    let size = f32::from_le_bytes(size) * scale;
    system.memory.write_bytes(at, &size.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a touch of the top screen's pause menu touches the bottom screen
    /// where the same thing is, by the edges too, and nothing beside what
    /// went to them.
    #[test]
    fn pause_menu_touches_follow_the_edges() {
        let mut aim_mouse = AimMouse { pause_drawn: Some(10), ..AimMouse::default() };
        // the main page drawn by the edges, Resume, Retreat, Options, Remove Display
        aim_mouse.edges_drawn = Some(10);
        assert_eq!(aim_mouse.pause_menu_touch(11, 60.0, 83.0), Some((60.0, 83.0)));
        assert_eq!(aim_mouse.pause_menu_touch(11, 330.0, 83.0), Some((250.0, 83.0)));
        assert_eq!(aim_mouse.pause_menu_touch(11, 200.0, 134.0), Some((160.0, 134.0)));
        assert_eq!(aim_mouse.pause_menu_touch(11, 340.0, 222.0), Some((260.0, 222.0)));
        // between Resume and Retreat, and left of Remove Display
        assert_eq!(aim_mouse.pause_menu_touch(11, 200.0, 83.0), None);
        assert_eq!(aim_mouse.pause_menu_touch(11, 250.0, 222.0), None);
        // a page of Options, all of it in the middle
        assert_eq!(aim_mouse.pause_menu_touch(20, 60.0, 83.0), Some((20.0, 83.0)));
        assert_eq!(aim_mouse.pause_menu_touch(20, 20.0, 83.0), None);
    }
}
