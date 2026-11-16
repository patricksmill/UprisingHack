//! Orbit of the Idol View on the Vault's Idols: a left-drag over the Idol
//! layer turns the game's own camera angle and tilt, and on release keeps
//! turning at the release speed, slowing as the game's own stylus momentum
//! does. The game's momentum only follows a real stylus release, so it is
//! kept here and written as angles each frame (analysis/FINDINGS.md,
//! "Idols view state").

use super::{read_word, System};

/// the viewer object's view state.
const PITCH: u32 = 0x170;
const YAW: u32 = 0x174;
/// 6 while the idol spins by itself; any input sets 0 for good.
const SPIN: u32 = 0x524;
/// the game holds tilt within its own limit, ±90°.
const PITCH_LIMIT: f32 = 90.0;
/// momentum lost each frame after release: about the game's own decay,
/// a turn slowing to rest over about a second.
const DECAY: f32 = 0.96;
/// slower than this, in degrees a frame, the idol is at rest.
const REST: f32 = 0.05;
/// frames of drag motion the release speed averages; a drag held still for
/// as long leaves no momentum.
const RECENT: usize = 3;

/// the Orbit gesture and its momentum. Distances are top-screen pixels of
/// the frontend's 400-wide screen until the hook turns them into degrees.
#[derive(Debug, Clone, Default)]
pub(super) struct IdolView {
    /// whether the frontend's button was down at its last pointer event.
    pub(super) held: bool,
    /// the last pointer point of a drag in progress.
    grab: Option<[f32; 2]>,
    /// pointer motion since the last frame the view was written.
    pending: [f32; 2],
    /// the last frames' motion, newest last.
    recent: Vec<[f32; 2]>,
    /// degrees a frame the view keeps turning after release.
    velocity: [f32; 2],
    /// whether this drag has stopped the idle spin yet.
    spin_stopped: bool,
}

impl IdolView {
    pub(super) fn dragging(&self) -> bool {
        self.grab.is_some()
    }

    /// starts an Orbit at a top-screen point.
    pub(super) fn begin(&mut self, point: [f32; 2]) {
        self.grab = Some(point);
        self.pending = [0.0; 2];
        self.recent.clear();
        self.velocity = [0.0; 2];
        self.spin_stopped = false;
    }

    /// the pointer moved, the button still held.
    pub(super) fn drag(&mut self, point: [f32; 2]) {
        if let Some(last) = self.grab.replace(point) {
            self.pending[0] += point[0] - last[0];
            self.pending[1] += point[1] - last[1];
        }
    }

    /// the button came up: the recent motion becomes momentum, in degrees.
    pub(super) fn release(&mut self, gain: f32) {
        if self.grab.take().is_none() { return; }
        let frames = self.recent.len().max(1) as f32;
        let sum = self.recent.iter().fold([0.0; 2], |sum, step| [sum[0] + step[0], sum[1] + step[1]]);
        self.velocity = [sum[0] / frames * gain, sum[1] / frames * gain];
        self.recent.clear();
    }

    /// stops any Orbit, as when the Screen goes.
    pub(super) fn cancel(&mut self) {
        *self = IdolView { held: self.held, ..IdolView::default() };
    }

    /// this frame's turn, in degrees: the drag's motion, or the momentum.
    fn step(&mut self, gain: f32) -> Option<[f32; 2]> {
        if self.grab.is_some() {
            let moved = std::mem::take(&mut self.pending);
            self.recent.push(moved);
            if self.recent.len() > RECENT { self.recent.remove(0); }
            return Some([moved[0] * gain, moved[1] * gain]);
        }
        if self.velocity[0].hypot(self.velocity[1]) < REST {
            self.velocity = [0.0; 2];
            return None;
        }
        let turn = self.velocity;
        self.velocity = [turn[0] * DECAY, turn[1] * DECAY];
        Some(turn)
    }
}

/// the game's save data, whose camera flags hold Reticle/Camera's choices.
const SAVE: u32 = 0x00D636C4;
const CAMERA_FLAGS: u32 = 0x163EC;
/// Land Battle's Invert Horizontal and Invert Vertical.
const LAND_INVERT: [u8; 2] = [0x02, 0x04];

/// the Land Battle camera's inversion, horizontal and vertical, as the
/// game reads it each frame.
fn land_invert(system: &mut System) -> [bool; 2] {
    let save = read_word(system, SAVE);
    if save == 0 || !system.memory.is_readable(save + CAMERA_FLAGS, 1) { return [false; 2]; }
    let mut flags = [0u8];
    system.memory.read_bytes(save + CAMERA_FLAGS, &mut flags);
    LAND_INVERT.map(|bit| flags[0] & bit != 0)
}

/// before the Idols viewer updates, the object in r0: turns its view by
/// this frame's Orbit. `gain` is degrees a top-screen pixel; the Land
/// Battle camera's inversion applies.
pub(super) fn update(system: &mut System, object: u32, gain: f32) {
    let invert = land_invert(system);
    let Some(mouse) = system.aim_mouse.as_mut() else { return };
    let Some(turn) = mouse.idol_view.step(gain) else { return };
    let stop_spin = mouse.idol_view.grab.is_some() && !std::mem::replace(&mut mouse.idol_view.spin_stopped, true);
    if stop_spin { system.memory.write_bytes(object + SPIN, &0u32.to_le_bytes()); }
    if turn == [0.0; 2] { return; }
    let sign = |inverted: bool| if inverted { -1.0 } else { 1.0 };
    let yaw = f32::from_bits(read_word(system, object + YAW)) + turn[0] * sign(invert[0]);
    let pitch = f32::from_bits(read_word(system, object + PITCH)) + turn[1] * sign(invert[1]);
    let (yaw, pitch) = (yaw.rem_euclid(360.0), pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT));
    system.memory.write_bytes(object + YAW, &yaw.to_le_bytes());
    system.memory.write_bytes(object + PITCH, &pitch.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_turns_by_its_motion_and_coasts_after_release() {
        let mut view = IdolView::default();
        view.begin([0.0, 0.0]);
        for x in 1..=4 {
            view.drag([x as f32 * 10.0, 0.0]);
            assert_eq!(view.step(0.3), Some([3.0, 0.0]));
        }
        view.release(0.3);
        let first = view.step(0.3).unwrap();
        assert!((first[0] - 3.0).abs() < 0.001, "momentum starts at the release speed");
        let mut frames = 1;
        while view.step(0.3).is_some() { frames += 1; }
        assert!((60..=120).contains(&frames), "rest after about a second, took {frames}");
    }

    #[test]
    fn a_drag_held_still_before_release_leaves_no_momentum() {
        let mut view = IdolView::default();
        view.begin([0.0, 0.0]);
        view.drag([50.0, 20.0]);
        view.step(0.3);
        for _ in 0..RECENT { view.step(0.3); }
        view.release(0.3);
        assert_eq!(view.step(0.3), None);
    }
}
