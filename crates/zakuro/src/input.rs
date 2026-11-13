//! keyboard to 3DS button mapping: the console's buttons for most titles,
//! Action Bindings for Kid Icarus: Uprising.

use winit::keyboard::{KeyCode, PhysicalKey};
use zakuro_core::services::hid::{InputState, PadState};
use zakuro_core::services::prompts::{Input, Label, MouseButton, Prompts};

use crate::bindings::{Action, ActionBindings, Context, Inputs, Source};
use crate::settings::Keys;

/// accumulates key state between frames.
#[derive(Default)]
pub struct Keyboard {
    keys: Keys,
    buttons: PadState,
    /// the circle pad keys held, up, down, left, right.
    circle: [bool; 4],
    /// the C-stick's, the same way.
    c_stick: [bool; 4],
    touch: Option<(u16, u16)>,
    /// buttons the mouse's buttons hold while it aims.
    mouse_buttons: PadState,
    /// how the mouse tilts the console, see InputState::tilt.
    tilt: [f32; 2],
    /// Kid Icarus's Action Bindings, which then take every key, mouse
    /// button and controller button in place of the console's.
    bindings: Option<ActionBindings>,
    actions: Inputs,
    context: Context,
}

impl Keyboard {
    pub fn new(keys: Keys) -> Keyboard {
        Keyboard { keys, ..Keyboard::default() }
    }

    pub fn set_keys(&mut self, keys: Keys) {
        self.keys = keys;
        self.release();
    }

    /// takes input through Action Bindings, or the console's buttons with none.
    pub fn set_bindings(&mut self, bindings: Option<ActionBindings>) {
        if bindings != self.bindings {
            self.bindings = bindings;
            self.release();
        }
    }

    pub fn acting(&self) -> bool {
        self.bindings.is_some()
    }

    /// where the game is, a battle or a Screen, for the actions of the
    /// inputs pressed from now on.
    pub fn set_context(&mut self, context: Context) {
        self.context = context;
    }

    /// lets go of everything, when the game stops seeing the keyboard.
    pub fn release(&mut self) {
        self.buttons = PadState::default();
        self.circle = [false; 4];
        self.c_stick = [false; 4];
        self.touch = None;
        self.mouse_buttons = PadState::default();
        self.tilt = [0.0; 2];
        self.actions.release_all();
    }

    /// holds or lets go of a button for a mouse button.
    pub fn mouse_button(&mut self, button: PadState, pressed: bool) {
        self.mouse_buttons.set(button, pressed);
    }

    /// presses or lets go of a key, mouse button or controller button
    /// through the Action Bindings, giving the actions a press starts.
    pub fn source(&mut self, source: Source, pressed: bool) -> Vec<Action> {
        let Some(bindings) = &self.bindings else { return Vec::new() };
        if !pressed {
            self.actions.release(source);
            return Vec::new();
        }
        self.actions.press(bindings, source, self.context)
    }

    /// lets go of the Action Bindings' inputs of one kind.
    pub fn release_where(&mut self, gone: impl Fn(Source) -> bool) {
        self.actions.release_where(gone);
    }

    fn button_for(&self, key: KeyCode) -> Option<PadState> {
        let keys = &self.keys;
        [
            (keys.a, PadState::A),
            (keys.b, PadState::B),
            (keys.x, PadState::X),
            (keys.y, PadState::Y),
            (keys.l, PadState::L),
            (keys.r, PadState::R),
            (keys.start, PadState::START),
            (keys.select, PadState::SELECT),
            (keys.up, PadState::UP),
            (keys.down, PadState::DOWN),
            (keys.left, PadState::LEFT),
            (keys.right, PadState::RIGHT),
            (keys.zl, PadState::ZL),
            (keys.zr, PadState::ZR),
        ]
        .into_iter()
        .find(|&(bound, _)| bound == key)
        .map(|(_, button)| button)
    }

    pub fn key(&mut self, key: PhysicalKey, pressed: bool) -> Vec<Action> {
        let PhysicalKey::Code(code) = key else {
            return Vec::new();
        };
        if self.acting() {
            return self.source(Source::Key(code), pressed);
        }
        if let Some(button) = self.button_for(code) {
            self.buttons.set(button, pressed);
        }
        let circle = [self.keys.circle_up, self.keys.circle_down, self.keys.circle_left, self.keys.circle_right];
        let c_stick = [self.keys.c_up, self.keys.c_down, self.keys.c_left, self.keys.c_right];
        for (held, bound) in self.circle.iter_mut().zip(circle).chain(self.c_stick.iter_mut().zip(c_stick)) {
            if bound == code {
                *held = pressed;
            }
        }
        Vec::new()
    }

    /// records a click on the bottom screen, in screen pixels, or its release.
    pub fn touch(&mut self, position: Option<(u16, u16)>) {
        self.touch = position;
    }

    /// records the tilt the mouse gives the console.
    pub fn tilt(&mut self, tilt: [f32; 2]) {
        self.tilt = tilt;
    }

    pub fn state(&self) -> InputState {
        // holding two opposite keys cancels out, which is what a real stick
        // would do
        let axis = |plus: bool, minus: bool| plus as i8 as f32 - minus as i8 as f32;
        let [up, down, left, right] = self.circle;
        let [c_up, c_down, c_left, c_right] = self.c_stick;
        let mut state = InputState {
            buttons: self.buttons | self.mouse_buttons,
            circle_x: axis(right, left),
            circle_y: axis(up, down),
            c_stick_x: axis(c_right, c_left),
            c_stick_y: axis(c_up, c_down),
            touch: self.touch,
            tilt: self.tilt,
            gyro: [0.0; 3],
        };
        if let Some(bindings) = &self.bindings {
            let held = self.actions.held(bindings, self.context);
            state.buttons = held.buttons;
            [state.circle_x, state.circle_y] = held.circle;
            [state.c_stick_x, state.c_stick_y] = held.c_stick;
        }
        state
    }

    /// whether the Gyro Aim action is held.
    pub fn gyro_held(&self) -> bool {
        self.bindings.as_ref().is_some_and(|bindings| self.actions.held(bindings, self.context).gyro_aim)
    }
}

/// the console's button symbols as the keys they are on.
pub fn prompts(keys: &Keys) -> Prompts {
    let mut prompts = Prompts::default();
    for (input, key) in [
        (Input::A, keys.a),
        (Input::B, keys.b),
        (Input::X, keys.x),
        (Input::Y, keys.y),
        (Input::L, keys.l),
        (Input::R, keys.r),
        (Input::ZL, keys.zl),
        (Input::ZR, keys.zr),
        (Input::Up, keys.up),
        (Input::Down, keys.down),
        (Input::Left, keys.left),
        (Input::Right, keys.right),
    ] {
        prompts.set(input, Label::Key(cap(key)));
    }
    let together = |keys: &[KeyCode]| Label::Key(keys.iter().map(|&key| cap(key)).collect::<Vec<_>>().join(""));
    prompts.set(Input::DPad, together(&[keys.up, keys.left, keys.down, keys.right]));
    prompts.set(Input::UpDown, together(&[keys.up, keys.down]));
    prompts.set(Input::LeftRight, together(&[keys.left, keys.right]));
    prompts.set(Input::CirclePad, together(&[keys.circle_up, keys.circle_left, keys.circle_down, keys.circle_right]));
    prompts
}

/// how a Kid Icarus Action Binding's key or mouse button looks in a prompt.
pub fn label(source: Source) -> Label {
    match source {
        Source::Key(key) => Label::Key(cap(key)),
        Source::Mouse(winit::event::MouseButton::Left) => Label::Mouse(MouseButton::Left),
        Source::Mouse(winit::event::MouseButton::Right) => Label::Mouse(MouseButton::Right),
        Source::Mouse(winit::event::MouseButton::Middle) => Label::Mouse(MouseButton::Middle),
        other => Label::Key(other.name()),
    }
}

/// a key's name short enough for a keycap.
fn cap(key: KeyCode) -> String {
    let short = match key {
        KeyCode::ArrowUp => "↑",
        KeyCode::ArrowDown => "↓",
        KeyCode::ArrowLeft => "←",
        KeyCode::ArrowRight => "→",
        KeyCode::Enter | KeyCode::NumpadEnter => "Enter",
        KeyCode::Backspace => "Bksp",
        KeyCode::Space => "Space",
        KeyCode::Escape => "Esc",
        KeyCode::Tab => "Tab",
        KeyCode::ShiftLeft | KeyCode::ShiftRight => "Shift",
        KeyCode::ControlLeft | KeyCode::ControlRight => "Ctrl",
        KeyCode::AltLeft | KeyCode::AltRight => "Alt",
        KeyCode::Minus => "-",
        KeyCode::Equal => "=",
        KeyCode::Comma => ",",
        KeyCode::Period => ".",
        KeyCode::Slash => "/",
        KeyCode::Semicolon => ";",
        KeyCode::Quote => "'",
        KeyCode::BracketLeft => "[",
        KeyCode::BracketRight => "]",
        _ => return crate::menus::key_name(key).trim_start_matches("Numpad").to_owned(),
    };
    short.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a prompt shows the key a button is bound to, and follows a rebinding.
    #[test]
    fn prompts_follow_the_keys() {
        let mut keys = Keys::default();
        let prompts = super::prompts(&keys);
        assert_eq!(prompts.get(Input::A), &Label::Key("X".into()));
        assert_eq!(prompts.get(Input::DPad), &Label::Key("↑←↓→".into()));
        assert_eq!(prompts.get(Input::CirclePad), &Label::Key("IJKL".into()));
        keys.a = KeyCode::Space;
        assert_eq!(super::prompts(&keys).get(Input::A), &Label::Key("Space".into()));
    }

    #[test]
    fn keys_press_the_buttons_they_are_bound_to() {
        let mut keyboard = Keyboard::new(Keys { a: KeyCode::KeyP, ..Keys::default() });
        keyboard.key(PhysicalKey::Code(KeyCode::KeyP), true);
        assert!(keyboard.state().buttons.contains(PadState::A));
        keyboard.key(PhysicalKey::Code(KeyCode::KeyX), true);
        assert!(!keyboard.state().buttons.contains(PadState::B), "X is no longer bound to anything");
    }

    #[test]
    fn the_c_stick_keys_push_the_c_stick() {
        let mut keyboard = Keyboard::new(Keys::default());
        keyboard.key(PhysicalKey::Code(KeyCode::KeyT), true);
        keyboard.key(PhysicalKey::Code(KeyCode::KeyF), true);
        keyboard.key(PhysicalKey::Code(KeyCode::Digit2), true);
        let state = keyboard.state();
        assert_eq!((state.c_stick_x, state.c_stick_y), (-1.0, 1.0));
        assert_eq!((state.circle_x, state.circle_y), (0.0, 0.0));
        assert!(state.buttons.contains(PadState::ZR));
    }

    #[test]
    fn opposite_circle_keys_cancel_out() {
        let mut keyboard = Keyboard::new(Keys::default());
        keyboard.key(PhysicalKey::Code(KeyCode::KeyJ), true);
        assert_eq!(keyboard.state().circle_x, -1.0);
        keyboard.key(PhysicalKey::Code(KeyCode::KeyL), true);
        assert_eq!(keyboard.state().circle_x, 0.0);
        keyboard.release();
        assert_eq!(keyboard.state().circle_x, 0.0);
    }
}
