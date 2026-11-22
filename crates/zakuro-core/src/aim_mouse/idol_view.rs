//! The Idol View on the Vault's Idols, driven through the game's own view
//! state (analysis/FINDINGS.md, "Idols view state"):
//! - Orbit: a left-drag over the Idol layer turns the camera angle and
//!   tilt, and keeps turning after release, slowing as the game's own stylus
//!   momentum does. The game's momentum only follows a real stylus release,
//!   so it is kept here.
//! - Orbit with a stick: the Idol View Orbit binding, by default the right
//!   stick, turns at up to 180°/s at full tilt, in place of the game's own
//!   C-stick Orbit.
//! - Pan: a right-drag moves the idol with the pointer.
//! - Zoom: the wheel, eased over a few frames.
//! - Reset View: a double-click puts back the view each idol was first
//!   shown with, its spin still off.

use super::{read_word, System};

/// the viewer object's view state.
const PITCH: u32 = 0x170;
const YAW: u32 = 0x174;
/// a byte, 6 while the idol spins by itself; any input sets 0 until the
/// idol changes. The bytes after it are not the switch's.
const SPIN: u32 = 0x524;
/// the idol shown, which moves with L/R and the List's Cursor; the game
/// sets that idol's first view the frame it changes.
const IDOL: u32 = 0x414;
const ZOOM: u32 = 0x168;
/// position, and the twins the picture is drawn from; the game holds both
/// within its own Pan limits each tick.
const POSITION: [u32; 2] = [0x178, 0x17C];
const POSITION_DRAWN: [u32; 2] = [0x184, 0x188];
/// native top-screen pixels a unit of position moves the idol, measured at
/// zoom 0 and 1: the inverse of the scale is about linear in zoom.
const PAN_PIXELS: [f32; 2] = [0.147, 0.049];
/// a wheel notch's Zoom goes in over about 80 ms: this share of what is
/// left each 30 Hz tick.
const ZOOM_EASE: f32 = 0.6;
/// the game holds tilt within its own limit, ±90°.
const PITCH_LIMIT: f32 = 90.0;
/// momentum lost each frame after release: about the game's own decay,
/// a turn slowing to rest over about a second.
const DECAY: f32 = 0.96;
/// degrees a viewer tick the Idol View Orbit binding turns at full tilt:
/// 180°/s at the viewer's 30 Hz.
pub(super) const STICK_ORBIT: f32 = 180.0 / 30.0;
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
    /// the Idol View Orbit binding's lean past its dead zone, right and up.
    pub(super) stick: [f32; 2],
    /// whether this drag has stopped the idle spin yet.
    spin_stopped: bool,
    /// a Pan in progress: its last point and the native pixels a top-screen
    /// pixel spans there.
    pan: Option<([f32; 2], [f32; 2])>,
    /// Pan motion not yet written, native pixels.
    pan_pending: [f32; 2],
    /// Zoom not yet written.
    zoom_pending: f32,
    /// the last press that began an Orbit: its frame and point.
    last_press: Option<(u64, [f32; 2])>,
    /// a double-click asked for Reset View.
    reset: bool,
    /// the view the shown idol was first shown with: the Screen object,
    /// the idol and the view words, in VIEW's order.
    first_view: Option<(u32, u32, [u32; VIEW.len()])>,
}

/// the view words Reset View puts back. The drawn position twins settle
/// after the idol changes, so they move with the position instead.
const VIEW: [u32; 5] = [ZOOM, PITCH, YAW, POSITION[0], POSITION[1]];
/// a second press this soon (60 Hz frames) and near (top-screen pixels)
/// after the first makes a double-click.
const DOUBLE_CLICK: (u64, f32) = (24, 6.0);

impl IdolView {
    pub(super) fn dragging(&self) -> bool {
        self.grab.is_some()
    }

    /// starts an Orbit at a top-screen point on a frame; a double-click
    /// also asks for Reset View.
    pub(super) fn press(&mut self, frame: u64, point: [f32; 2]) {
        let double = self.last_press.is_some_and(|(at, from)| frame.saturating_sub(at) <= DOUBLE_CLICK.0
            && (point[0] - from[0]).hypot(point[1] - from[1]) <= DOUBLE_CLICK.1);
        self.last_press = (!double).then_some((frame, point));
        self.begin(point);
        self.reset |= double;
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

    pub(super) fn panning(&self) -> bool {
        self.pan.is_some()
    }

    pub(super) fn begin_pan(&mut self, point: [f32; 2], scale: [f32; 2]) {
        self.pan = Some((point, scale));
        self.pan_pending = [0.0; 2];
        self.velocity = [0.0; 2];
    }

    pub(super) fn pan_to(&mut self, point: [f32; 2]) {
        let Some((last, scale)) = self.pan.as_mut() else { return };
        for axis in 0..2 { self.pan_pending[axis] += (point[axis] - last[axis]) * scale[axis]; }
        *last = point;
    }

    pub(super) fn end_pan(&mut self) {
        self.pan = None;
    }

    /// Zoom by a share of the game's 0..1, in positive.
    pub(super) fn zoom_by(&mut self, amount: f32) {
        self.zoom_pending = (self.zoom_pending + amount).clamp(-1.0, 1.0);
    }

    /// this tick's Zoom, eased.
    fn zoom_step(&mut self) -> f32 {
        if self.zoom_pending.abs() < 0.002 { return std::mem::take(&mut self.zoom_pending); }
        let step = self.zoom_pending * ZOOM_EASE;
        self.zoom_pending -= step;
        step
    }

    /// stops any Orbit, as when the Screen goes.
    pub(super) fn cancel(&mut self) {
        *self = IdolView { held: self.held, stick: self.stick, first_view: self.first_view, ..IdolView::default() };
    }

    /// this frame's turn, in degrees: the drag's motion, or the momentum.
    fn step(&mut self, gain: f32) -> Option<[f32; 2]> {
        if self.grab.is_some() {
            let moved = std::mem::take(&mut self.pending);
            self.recent.push(moved);
            if self.recent.len() > RECENT { self.recent.remove(0); }
            return Some([moved[0] * gain, moved[1] * gain]);
        }
        if self.stick != [0.0; 2] {
            // stick up raises the view as a drag up does
            self.velocity = [0.0; 2];
            return Some([self.stick[0] * STICK_ORBIT, -self.stick[1] * STICK_ORBIT]);
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
    let view = &mut mouse.idol_view;
    let turn = view.step(gain);
    let pan = std::mem::take(&mut view.pan_pending);
    let zoom = view.zoom_step();
    let acting = view.grab.is_some() || view.stick != [0.0; 2] || view.pan.is_some() || zoom != 0.0;
    let stop_spin = acting && !std::mem::replace(&mut view.spin_stopped, true);
    if !acting && turn.is_none() { view.spin_stopped = false; }
    let read = |system: &mut System, offset: u32| f32::from_bits(read_word(system, object + offset));
    let write = |system: &mut System, offset: u32, value: f32| system.memory.write_bytes(object + offset, &value.to_le_bytes());
    let reset = std::mem::take(&mut view.reset);
    if reset {
        view.velocity = [0.0; 2];
        view.zoom_pending = 0.0;
        view.pending = [0.0; 2];
    }
    if stop_spin { system.memory.write_bytes(object + SPIN, &[0]); }
    let idol = read_word(system, object + IDOL);
    let Some(mouse) = system.aim_mouse.as_mut() else { return };
    let remembered = mouse.idol_view.first_view.filter(|&(screen, shown, _)| screen == object && shown == idol);
    let Some((_, _, first)) = remembered else {
        let words = VIEW.map(|offset| read_word(system, object + offset));
        if let Some(mouse) = system.aim_mouse.as_mut() { mouse.idol_view.first_view = Some((object, idol, words)); }
        return;
    };
    if reset {
        for axis in 0..2 {
            let moved = f32::from_bits(first[3 + axis]) - read(system, POSITION[axis]);
            let drawn = read(system, POSITION_DRAWN[axis]) + moved;
            write(system, POSITION_DRAWN[axis], drawn);
        }
        for (offset, word) in VIEW.into_iter().zip(first) { system.memory.write_bytes(object + offset, &word.to_le_bytes()); }
        return;
    }
    if let Some(turn) = turn.filter(|turn| *turn != [0.0; 2]) {
        let sign = |inverted: bool| if inverted { -1.0 } else { 1.0 };
        let yaw = read(system, YAW) + turn[0] * sign(invert[0]);
        let pitch = read(system, PITCH) + turn[1] * sign(invert[1]);
        write(system, YAW, yaw.rem_euclid(360.0));
        write(system, PITCH, pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT));
    }
    if zoom != 0.0 {
        let level = read(system, ZOOM);
        let next = (level + zoom).clamp(0.0, 1.0);
        write(system, ZOOM, next);
        if next != level + zoom {
            if let Some(mouse) = system.aim_mouse.as_mut() { mouse.idol_view.zoom_pending = 0.0; }
        }
    }
    if pan != [0.0; 2] {
        // the idol keeps the grabbed point under the pointer: screen down is
        // position down
        let units = pan_units(read(system, ZOOM), pan);
        for (axis, offset) in [(0, POSITION[0]), (1, POSITION[1]), (0, POSITION_DRAWN[0]), (1, POSITION_DRAWN[1])] {
            let value = read(system, offset) + units[axis];
            write(system, offset, value);
        }
    }
}

/// native top-screen pixels of Pan, right and down, as position at a zoom.
fn pan_units(zoom: f32, pixels: [f32; 2]) -> [f32; 2] {
    let per_pixel = PAN_PIXELS[0] + (PAN_PIXELS[1] - PAN_PIXELS[0]) * zoom.clamp(0.0, 1.0);
    [pixels[0] * per_pixel, -pixels[1] * per_pixel]
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

    #[test]
    fn a_wheel_notch_eases_in_over_a_few_frames() {
        let mut view = IdolView::default();
        view.zoom_by(0.1);
        let steps: Vec<f32> = std::iter::from_fn(|| Some(view.zoom_step()).filter(|step| *step != 0.0)).collect();
        assert!((steps.iter().sum::<f32>() - 0.1).abs() < 1e-6);
        assert!((3..=8).contains(&steps.len()), "{steps:?}");
    }

    #[test]
    fn a_pan_moves_by_the_measured_scale_and_screen_down_is_position_down() {
        let units = pan_units(0.5, [10.2, 10.2]);
        assert!((units[0] - 1.0).abs() < 0.03 && (units[1] + 1.0).abs() < 0.03, "{units:?}");
    }

    #[test]
    fn a_full_stick_turns_180_degrees_a_second_and_stops_at_once() {
        let mut view = IdolView::default();
        view.stick = [1.0, 0.0];
        let turned: f32 = (0..30).map(|_| view.step(0.3).unwrap()[0]).sum();
        assert!((turned - 180.0).abs() < 0.01, "{turned}");
        view.stick = [0.0, 1.0];
        assert_eq!(view.step(0.3), Some([0.0, -STICK_ORBIT]), "stick up lowers the tilt, as a drag up does");
        view.stick = [0.0; 2];
        assert_eq!(view.step(0.3), None, "no momentum after the stick");
    }

    #[test]
    fn a_second_press_soon_and_near_is_a_double_click() {
        let mut view = IdolView::default();
        view.press(100, [50.0, 50.0]);
        view.release(0.3);
        assert!(!view.reset);
        view.press(110, [52.0, 51.0]);
        assert!(view.reset, "a double-click");
        view.reset = false;
        view.release(0.3);
        view.press(115, [52.0, 51.0]);
        assert!(!view.reset, "a third press starts over");
        view.release(0.3);
        view.press(200, [52.0, 51.0]);
        assert!(!view.reset, "too late");
    }
}
