//! mouse aiming and a smaller HUD for Kid Icarus: Uprising (USA), a test of
//! what a port's overrides will do: the mouse's movement goes straight into
//! the aim delta the game's touch handling produces, with none of its
//! smoothing or flick, and the top screen's HUD and reticle are drawn
//! smaller. only the interpreter stops where this looks, so the functions
//! these places are in run in it, see FUNCTIONS.

use crate::System;

/// Kid Icarus: Uprising, the USA cartridge.
pub const PROGRAM_ID: u64 = 0x0004_0000_0003_0100;

/// the functions the places below are in, which have to run in the
/// interpreter for it to stop there: the aim delta's and the camera box's
/// callers, the sprite flush and the reticle's placing. the rest of the
/// game can stay recompiled.
pub const FUNCTIONS: [u32; 5] = [0x006D269C, 0x006D2D8C, 0x0092F408, 0x007C4854, 0x0060D39C];

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

/// what the CPU held where the narration's line was drawn in the game's
/// place, put back when the call returns there, and where the line was.
struct Saved {
    regs: [u32; 16],
    cpsr: zakuro_cpu::psr::Psr,
    vfp: [u32; 32],
    message: u32,
    position: [u8; 8],
}

/// where placing the reticle, 0x007C4854, has worked out the size to draw
/// it at, in s17, its base size over its depth, and is about to keep it.
const RETICLE_SIZE: u32 = 0x007C4BD0;

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
    /// the 2D context the top HUD was last drawn through.
    context: Option<u32>,
    /// the CPU while the narration's line is drawn on the top screen.
    saved: Option<Saved>,
}

impl Default for AimMouse {
    fn default() -> AimMouse {
        AimMouse { pending: [0.0; 2], captured: false, hud_scale: 1.0, narration_on_top: false, context: None, saved: None }
    }
}

impl AimMouse {
    /// for the game, drawing the HUD at hud_scale and the narration on the
    /// top screen too when narration_on_top.
    pub fn new(hud_scale: f32, narration_on_top: bool) -> AimMouse {
        AimMouse { hud_scale, narration_on_top, ..AimMouse::default() }
    }

    pub fn add(&mut self, dx: f32, dy: f32) {
        self.pending[0] += dx;
        self.pending[1] += dy;
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
        RETICLE_SIZE => shrink_reticle(system),
        TOP_UI_AFTER_HUD => narration_on_top(system),
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

/// draws the narration's line on the top screen as well, centred at its
/// bottom: the game's own drawing of it, called here as if the pass called
/// it, with the line moved for the call. back here, everything is put back
/// and the pass goes on.
fn narration_on_top(system: &mut System) {
    let Some(aim_mouse) = &mut system.aim_mouse else { return };
    if let Some(back) = aim_mouse.saved.take() {
        system.cpu.regs = back.regs;
        system.cpu.cpsr = back.cpsr;
        system.cpu.vfp.regs = back.vfp;
        system.memory.write_bytes(back.message + 0x14, &back.position);
        return;
    }
    let (true, Some(context)) = (aim_mouse.narration_on_top, aim_mouse.context) else { return };
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
    let cpu = &mut system.cpu;
    let saved = Saved { regs: cpu.regs, cpsr: cpu.cpsr, vfp: cpu.vfp.regs, message, position };
    cpu.regs[0] = message;
    cpu.regs[1] = context;
    cpu.regs[14] = TOP_UI_AFTER_HUD;
    cpu.regs[15] = DRAW_MESSAGE_LINE;
    cpu.cpsr.thumb = false;
    if let Some(aim_mouse) = &mut system.aim_mouse {
        aim_mouse.saved = Some(saved);
    }
}

/// the top screen's battle HUD, by where its sprite flushes return to:
/// score, health bar, wings, status icon. found in the USA build's air and
/// ground battles, other HUD stays the size the game draws it.
const TOP_HUD: [u32; 7] = [0x009F2F90, 0x009F45FC, 0x009F4800, 0x009F4BD4, 0x009F52C8, 0x009F557C, 0x00725828];

/// whether a sprite batch flushed from caller draws the top screen's HUD,
/// which the narration's line is part of while it is drawn there.
fn top_hud(system: &System, caller: u32) -> bool {
    TOP_HUD.contains(&caller) || system.aim_mouse.as_ref().is_some_and(|aim_mouse| aim_mouse.saved.is_some())
}

/// draws a top screen HUD element smaller: the quads the flush is about to
/// draw, scaled together toward the edge or corner they sit by, or their
/// own middle when they are by none, like a marker on an enemy.
fn shrink_hud(system: &mut System) {
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
    let narration = system.aim_mouse.as_ref().is_some_and(|aim_mouse| aim_mouse.saved.is_some());
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

/// draws the reticle at the HUD's scale.
fn shrink_reticle(system: &mut System) {
    let scale = system.aim_mouse.as_ref().map_or(1.0, |aim_mouse| aim_mouse.hud_scale);
    let size = &mut system.cpu.vfp.regs[17];
    *size = (f32::from_bits(*size) * scale).to_bits();
}
