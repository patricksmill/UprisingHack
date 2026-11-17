//! game controllers, any the system knows, read thru gilrs,
//! plus the Switch's Pro Controller on macOS read from its reports, see pro_controller

use gilrs::{Axis, Button, EventType, Gamepad, Gilrs};
use zakuro_core::services::hid::{InputState, PadState};
use zakuro_core::services::prompts::{Direction, Input, Label, Prompts};

use crate::pro_controller::{is_pro_controller, Controllers, Report};
use crate::settings::PadButtons;

/// how far a stick has to lean before it counts by default, some rest a
/// little off center.
pub const DEADZONE: f32 = 0.15;

/// how far a stick has to lean before the player counts as using the
/// controller rather than the keyboard.
const USE_LEAN: f32 = 0.5;

/// the names a controller's face buttons have printed on them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Faces {
    /// A at the bottom, B right, X left, Y top, as Xbox and most others.
    Letters,
    /// cross, circle, square and triangle.
    PlayStation,
}

impl Faces {
    fn of(name: &str) -> Faces {
        let name = name.to_ascii_lowercase();
        let sony = ["playstation", "dualshock", "dualsense", "ps3", "ps4", "ps5", "sony"];
        if sony.iter().any(|word| name.contains(word)) { Faces::PlayStation } else { Faces::Letters }
    }
}

pub struct Gamepads {
    /// none when the system couldn't be asked for controllers
    gilrs: Option<Gilrs>,
    buttons: PadButtons,
    /// the controller pressed or leaned since the last look, see used.
    used: Option<Faces>,
    /// the d-pad directions an axis d-pad holds, up, down, left, right, so
    /// it presses and lets go as a d-pad of buttons does.
    dpad_axes: [bool; 4],
    /// a controller went away since the last look, see disconnected.
    gone: bool,
    /// Pro Controllers read from their own reports, on macOS
    pro: Controllers,
    /// the buttons they held at the last poll, to tell which got pressed and let go since
    pro_held: Vec<Button>,
}

/// how the sticks read: how far one leans before it counts, and how far the
/// right one turns, 1 as it leans.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sticks {
    pub dead_zone: f32,
    pub right_scale: f32,
}

impl Default for Sticks {
    fn default() -> Self {
        Sticks { dead_zone: DEADZONE, right_scale: 1.0 }
    }
}

/// is gilrs's reading of a controller left out, a Pro Controller's while one is read from its reports
fn left_out(pad: &Gamepad, reports: &[Report]) -> bool {
    !reports.is_empty() && is_pro_controller(pad.vendor_id(), pad.product_id(), pad.name())
}

impl Gamepads {
    pub fn new(buttons: PadButtons) -> Gamepads {
        let gilrs = Gilrs::new()
            .inspect_err(|error| log::warn!("controllers are not available, {error}"))
            .ok();
        Gamepads { gilrs, buttons, used: None, dpad_axes: [false; 4], gone: false, pro: Controllers::new(), pro_held: Vec::new() }
    }

    pub fn set_buttons(&mut self, buttons: PadButtons) {
        self.buttons = buttons;
    }

    /// takes in what the controllers did since the last call, and returns
    /// the buttons pressed (true) and let go (false) meanwhile, an axis
    /// d-pad's as its buttons, and a Pro Controller's read from its reports.
    pub fn poll(&mut self) -> Vec<(Button, bool)> {
        let mut changed = Vec::new();
        let reports = self.pro.reports();
        if let Some(gilrs) = &mut self.gilrs {
            while let Some(event) = gilrs.next_event() {
                // a Pro Controller read from its reports isn't read thru gilrs too
                let left_out = left_out(&gilrs.gamepad(event.id), &reports);
                match event.event {
                    EventType::ButtonPressed(button, _) if !left_out => {
                        changed.push((button, true));
                        self.used = Some(Faces::of(gilrs.gamepad(event.id).name()));
                    }
                    EventType::ButtonReleased(button, _) if !left_out => changed.push((button, false)),
                    EventType::AxisChanged(axis @ (Axis::DPadX | Axis::DPadY), value, _) if !left_out => {
                        let ways = match axis {
                            Axis::DPadY => [(0, Button::DPadUp, value > 0.5), (1, Button::DPadDown, value < -0.5)],
                            _ => [(2, Button::DPadLeft, value < -0.5), (3, Button::DPadRight, value > 0.5)],
                        };
                        for (at, button, held) in ways {
                            if self.dpad_axes[at] != held {
                                self.dpad_axes[at] = held;
                                changed.push((button, held));
                            }
                        }
                        if value.abs() > USE_LEAN {
                            self.used = Some(Faces::of(gilrs.gamepad(event.id).name()));
                        }
                    }
                    EventType::AxisChanged(_, value, _) if !left_out && value.abs() > USE_LEAN => {
                        self.used = Some(Faces::of(gilrs.gamepad(event.id).name()));
                    }
                    EventType::Connected => {
                        log::info!("controller connected, {}", gilrs.gamepad(event.id).name());
                    }
                    EventType::Disconnected => {
                        log::info!("controller disconnected");
                        self.gone = true;
                        self.dpad_axes = [false; 4];
                    }
                    _ => {}
                }
            }
        }
        // a Pro Controller's buttons pressed and let go since the last poll, as its reports have them
        let held: Vec<Button> = reports.iter().flat_map(|report| report.buttons.iter().copied()).collect();
        if held.iter().any(|button| !self.pro_held.contains(button)) {
            self.used = Some(Faces::Letters);
        }
        changed.extend(held.iter().filter(|button| !self.pro_held.contains(button)).map(|&button| (button, true)));
        changed.extend(self.pro_held.iter().filter(|button| !held.contains(button)).map(|&button| (button, false)));
        self.pro_held = held;
        changed
    }

    /// whether a controller went away since the last call, whose buttons
    /// then never let go.
    pub fn disconnected(&mut self) -> bool {
        std::mem::take(&mut self.gone)
    }

    /// the face buttons of the controller used since the last call, none
    /// when none was.
    pub fn used(&mut self) -> Option<Faces> {
        self.used.take()
    }

    /// the console's button symbols as the controller's buttons they are on.
    pub fn prompts(&self, faces: Faces) -> Prompts {
        let mut prompts = Prompts::default();
        let buttons = &self.buttons;
        for (input, button) in [
            (Input::A, buttons.a),
            (Input::B, buttons.b),
            (Input::X, buttons.x),
            (Input::Y, buttons.y),
            (Input::L, buttons.l),
            (Input::R, buttons.r),
            (Input::ZL, buttons.zl),
            (Input::ZR, buttons.zr),
        ] {
            prompts.set(input, label(button, faces));
        }
        prompts.set(Input::Home, label(Button::Mode, faces));
        // the d-pad as a whole, or one way, on its own buttons
        let pad = [buttons.up, buttons.down, buttons.left, buttons.right];
        if pad == [Button::DPadUp, Button::DPadDown, Button::DPadLeft, Button::DPadRight] {
            prompts.set(Input::DPad, Label::Cross(None));
        } else {
            for (input, button) in [(Input::Up, buttons.up), (Input::Down, buttons.down), (Input::Left, buttons.left), (Input::Right, buttons.right)] {
                prompts.set(input, label(button, faces));
            }
        }
        prompts.set(Input::CirclePad, Label::Stick("L".into()));
        prompts
    }

    /// does a controller hold button down
    pub fn holding(&self, button: Button) -> bool {
        let reports = self.pro.reports();
        reports.iter().any(|report| report.buttons.contains(&button))
            || self.gilrs.as_ref().is_some_and(|gilrs| {
                gilrs.gamepads().any(|(_, pad)| !left_out(&pad, &reports) && pad.is_pressed(button))
            })
    }

    /// what the controllers hold down, over what the keyboard does: their
    /// buttons as the console's unless Action Bindings take them, and the
    /// sticks as the circle pad and C-stick.
    pub fn apply(&self, mut state: InputState, sticks: Sticks, acting: bool) -> InputState {
        let reports = self.pro.reports();
        for report in &reports {
            if !acting {
                for (button, bound) in self.buttons.map() {
                    if report.buttons.contains(&bound) {
                        state.buttons.set(button, true);
                    }
                }
            }
            lean_sticks(&mut state, report.left, report.right, sticks);
        }
        let Some(gilrs) = &self.gilrs else { return state };
        for (_, pad) in gilrs.gamepads() {
            if left_out(&pad, &reports) {
                continue;
            }
            if !acting {
                for (button, bound) in self.buttons.map() {
                    if pad.is_pressed(bound) {
                        state.buttons.set(button, true);
                    }
                }
                // the d-pad of some controllers is a pair of axes
                let dpad = [
                    (PadState::RIGHT, pad.value(Axis::DPadX) > 0.5),
                    (PadState::LEFT, pad.value(Axis::DPadX) < -0.5),
                    (PadState::UP, pad.value(Axis::DPadY) > 0.5),
                    (PadState::DOWN, pad.value(Axis::DPadY) < -0.5),
                ];
                for (button, held) in dpad {
                    if held {
                        state.buttons.set(button, true);
                    }
                }
            }
            let left = [pad.value(Axis::LeftStickX), pad.value(Axis::LeftStickY)];
            let right = [pad.value(Axis::RightStickX), pad.value(Axis::RightStickY)];
            lean_sticks(&mut state, left, right, sticks);
        }
        state
    }
}

/// a stick's lean past the dead zone, scaled, none inside it. the dead
/// zone is taken out, so the stick starts from nothing at its edge rather
/// than jumping to it.
fn lean((x, y): (f32, f32), dead_zone: f32, scale: f32) -> Option<(f32, f32)> {
    let length = x.hypot(y);
    let dead_zone = dead_zone.clamp(0.0, 0.9);
    if length <= dead_zone {
        return None;
    }
    let past = ((length.min(1.0) - dead_zone) / (1.0 - dead_zone)) * scale / length;
    Some(((x * past).clamp(-1.0, 1.0), (y * past).clamp(-1.0, 1.0)))
}

/// the left stick is the circle pad and the right one the C-stick, up
/// positive as the console has it, each once it leans past the dead zone
fn lean_sticks(state: &mut InputState, left: [f32; 2], right: [f32; 2], sticks: Sticks) {
    if let Some((x, y)) = lean((left[0], left[1]), sticks.dead_zone, 1.0) {
        (state.circle_x, state.circle_y) = (x, y);
    }
    if let Some((x, y)) = lean((right[0], right[1]), sticks.dead_zone, sticks.right_scale) {
        (state.c_stick_x, state.c_stick_y) = (x, y);
    }
}

/// a controller button as a person would call it.
pub fn button_name(button: Button) -> &'static str {
    match button {
        Button::South => "Bottom face",
        Button::East => "Right face",
        Button::North => "Top face",
        Button::West => "Left face",
        Button::LeftTrigger => "LB / L1",
        Button::RightTrigger => "RB / R1",
        Button::LeftTrigger2 => "LT / L2",
        Button::RightTrigger2 => "RT / R2",
        Button::Select => "Back / Select",
        Button::Start => "Start",
        Button::Mode => "Home",
        Button::LeftThumb => "Left stick",
        Button::RightThumb => "Right stick",
        Button::DPadUp => "D-pad up",
        Button::DPadDown => "D-pad down",
        Button::DPadLeft => "D-pad left",
        Button::DPadRight => "D-pad right",
        Button::C => "C",
        Button::Z => "Z",
        Button::Unknown => "unknown",
    }
}

/// how a controller button looks in a prompt.
pub fn label(button: Button, faces: Faces) -> Label {
    let round = |name: &str| Label::Round(name.into());
    let shoulder = |name: &str| Label::Shoulder(name.into());
    match (button, faces) {
        (Button::South, Faces::Letters) => round("A"),
        (Button::East, Faces::Letters) => round("B"),
        (Button::West, Faces::Letters) => round("X"),
        (Button::North, Faces::Letters) => round("Y"),
        (Button::South, Faces::PlayStation) => round("×"),
        (Button::East, Faces::PlayStation) => round("○"),
        (Button::West, Faces::PlayStation) => round("□"),
        (Button::North, Faces::PlayStation) => round("△"),
        (Button::LeftTrigger, Faces::Letters) => shoulder("LB"),
        (Button::RightTrigger, Faces::Letters) => shoulder("RB"),
        (Button::LeftTrigger2, Faces::Letters) => shoulder("LT"),
        (Button::RightTrigger2, Faces::Letters) => shoulder("RT"),
        (Button::LeftTrigger, Faces::PlayStation) => shoulder("L1"),
        (Button::RightTrigger, Faces::PlayStation) => shoulder("R1"),
        (Button::LeftTrigger2, Faces::PlayStation) => shoulder("L2"),
        (Button::RightTrigger2, Faces::PlayStation) => shoulder("R2"),
        (Button::LeftThumb, _) => Label::Stick("L".into()),
        (Button::RightThumb, _) => Label::Stick("R".into()),
        (Button::DPadUp, _) => Label::Cross(Some(Direction::Up)),
        (Button::DPadDown, _) => Label::Cross(Some(Direction::Down)),
        (Button::DPadLeft, _) => Label::Cross(Some(Direction::Left)),
        (Button::DPadRight, _) => Label::Cross(Some(Direction::Right)),
        (Button::Select, Faces::Letters) => shoulder("Back"),
        (Button::Start, Faces::Letters) => shoulder("Start"),
        (Button::Select, Faces::PlayStation) => shoulder("Share"),
        (Button::Start, Faces::PlayStation) => shoulder("Opt"),
        (Button::Mode, _) => Label::Home,
        (Button::C, _) => round("C"),
        (Button::Z, _) => round("Z"),
        (Button::Unknown, _) => round("?"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// w/ no controller, or no way to ask for one, the keyboard's input goes thru as it was
    #[test]
    fn no_controller_leaves_the_keyboard_alone() {
        let gamepads = Gamepads::new(PadButtons::default());
        if gamepads.gilrs.as_ref().is_some_and(|gilrs| gilrs.gamepads().next().is_some()) {
            return;
        }
        let state = InputState { buttons: PadState::A, circle_x: 1.0, circle_y: 0.0, touch: Some((1, 2)), ..InputState::default() };
        let applied = gamepads.apply(state, Sticks::default(), false);
        assert_eq!(applied.buttons, PadState::A);
        assert_eq!((applied.circle_x, applied.circle_y, applied.touch), (1.0, 0.0, Some((1, 2))));
    }

    /// a prompt shows the controller button the console's is bound to, by
    /// what is printed on it, and follows a rebinding.
    #[test]
    fn prompts_follow_the_bindings() {
        let mut gamepads = Gamepads::new(PadButtons::default());
        let prompts = gamepads.prompts(Faces::Letters);
        // the console's A is on the right face button, B on a controller
        assert_eq!(prompts.get(Input::A), &Label::Round("B".into()));
        assert_eq!(prompts.get(Input::ZR), &Label::Shoulder("RT".into()));
        assert_eq!(gamepads.prompts(Faces::PlayStation).get(Input::A), &Label::Round("○".into()));
        gamepads.set_buttons(PadButtons { a: Button::South, up: Button::North, ..PadButtons::default() });
        let prompts = gamepads.prompts(Faces::Letters);
        assert_eq!(prompts.get(Input::A), &Label::Round("A".into()));
        assert_eq!(prompts.get(Input::DPad), &Input::DPad.console(), "a d-pad apart stays the console's");
        assert_eq!(prompts.get(Input::Up), &Label::Round("Y".into()));
    }

    #[test]
    fn sony_controllers_have_shapes() {
        assert_eq!(Faces::of("Sony Interactive Entertainment DualSense Wireless Controller"), Faces::PlayStation);
        assert_eq!(Faces::of("PS4 Controller"), Faces::PlayStation);
        assert_eq!(Faces::of("Xbox Wireless Controller"), Faces::Letters);
    }
}
