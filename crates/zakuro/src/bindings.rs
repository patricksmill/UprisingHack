//! Kid Icarus: Uprising's Action Bindings: keys, mouse buttons and
//! controller buttons assigned to what the player does, Attack or Use Power,
//! rather than to the console's buttons. Each action has a context, battle
//! or Screen, and turns into the console's input only there, so one key can
//! be Back in a Screen and Pause in a battle.

use std::collections::BTreeMap;

use gilrs::Button;
use serde::{Deserialize, Serialize};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;
use zakuro_core::services::hid::PadState;
use zakuro_core::services::prompts::{Direction, Input, Label, Prompts};

/// where an action does something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Context {
    /// a battle, its UI drawn and not paused.
    Gameplay,
    /// every Screen, Pause and the scenes between battles.
    #[default]
    Screens,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    /// held, the movement keys lean the circle pad only part way, so Pit
    /// walks instead of dashing and sprinting. Keyboard only: a stick
    /// already leans as far as it is pushed.
    Walk,
    AimUp,
    AimDown,
    AimLeft,
    AimRight,
    Attack,
    UsePower,
    /// the D-pad in a battle, which opens and moves through the powers.
    PowerUp,
    PowerDown,
    PowerLeft,
    PowerRight,
    VehicleAbility,
    ExitVehicle,
    /// the game's X in a battle, its own hint's "Change View".
    ChangeView,
    /// held, the gyro aims, when its mode is Hold.
    GyroAim,
    Pause,
    NavigateUp,
    NavigateDown,
    NavigateLeft,
    NavigateRight,
    Confirm,
    Back,
    /// Start outside a battle, which skips a scene.
    Skip,
    /// on Idols, L and R: the idol before and after.
    PreviousIdol,
    NextIdol,
    /// on Idols, X and Y: the Idol View's Zoom.
    ZoomIn,
    ZoomOut,
}

impl Action {
    pub const ALL: [Action; 31] = [
        Action::MoveUp,
        Action::MoveDown,
        Action::MoveLeft,
        Action::MoveRight,
        Action::Walk,
        Action::AimUp,
        Action::AimDown,
        Action::AimLeft,
        Action::AimRight,
        Action::Attack,
        Action::UsePower,
        Action::PowerUp,
        Action::PowerDown,
        Action::PowerLeft,
        Action::PowerRight,
        Action::VehicleAbility,
        Action::ExitVehicle,
        Action::ChangeView,
        Action::GyroAim,
        Action::Pause,
        Action::NavigateUp,
        Action::NavigateDown,
        Action::NavigateLeft,
        Action::NavigateRight,
        Action::Confirm,
        Action::Back,
        Action::Skip,
        Action::PreviousIdol,
        Action::NextIdol,
        Action::ZoomIn,
        Action::ZoomOut,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Action::MoveUp => "Move Forward",
            Action::MoveDown => "Move Back",
            Action::MoveLeft => "Move Left",
            Action::MoveRight => "Move Right",
            Action::Walk => "Walk (hold)",
            Action::AimUp => "Aim Up",
            Action::AimDown => "Aim Down",
            Action::AimLeft => "Aim Left",
            Action::AimRight => "Aim Right",
            Action::Attack => "Attack",
            Action::UsePower => "Use Power",
            Action::PowerUp => "Powers Up",
            Action::PowerDown => "Powers Down",
            Action::PowerLeft => "Powers Left",
            Action::PowerRight => "Powers Right",
            Action::VehicleAbility => "Vehicle Ability",
            Action::ExitVehicle => "Exit Vehicle",
            Action::ChangeView => "Change View",
            Action::GyroAim => "Gyro Aim (hold)",
            Action::Pause => "Pause",
            Action::NavigateUp => "Menu Up",
            Action::NavigateDown => "Menu Down",
            Action::NavigateLeft => "Menu Left",
            Action::NavigateRight => "Menu Right",
            Action::Confirm => "Confirm",
            Action::Back => "Back",
            Action::Skip => "Skip Scene",
            Action::PreviousIdol => "Previous Idol",
            Action::NextIdol => "Next Idol",
            Action::ZoomIn => "Zoom In",
            Action::ZoomOut => "Zoom Out",
        }
    }

    /// the contexts the action does something in.
    pub fn contexts(self) -> &'static [Context] {
        use Action::*;
        match self {
            MoveUp | MoveDown | MoveLeft | MoveRight => &[Context::Gameplay, Context::Screens],
            NavigateUp | NavigateDown | NavigateLeft | NavigateRight | Confirm | Back | Skip
            | PreviousIdol | NextIdol | ZoomIn | ZoomOut => &[Context::Screens],
            _ => &[Context::Gameplay],
        }
    }

    pub fn active(self, context: Context) -> bool {
        self.contexts().contains(&context)
    }

    /// whether a device can hold the action: Walk is the keyboard's alone.
    pub fn on(self, device: Device) -> bool {
        self != Action::Walk || device == Device::KeyboardMouse
    }

    /// whether two actions can be wanted at the same time, so cannot share
    /// an input.
    pub fn overlaps(self, other: Action) -> bool {
        self.contexts().iter().any(|&context| other.active(context))
    }

    /// the console's button the action holds, if it holds one.
    fn button(self) -> Option<PadState> {
        use Action::*;
        Some(match self {
            Attack => PadState::L,
            ChangeView => PadState::X,
            PowerUp | NavigateUp => PadState::UP,
            PowerDown | NavigateDown => PadState::DOWN,
            PowerLeft | NavigateLeft => PadState::LEFT,
            PowerRight | NavigateRight => PadState::RIGHT,
            Pause | Skip => PadState::START,
            Confirm => PadState::A,
            Back => PadState::B,
            PreviousIdol => PadState::L,
            NextIdol => PadState::R,
            ZoomIn => PadState::X,
            ZoomOut => PadState::Y,
            _ => return None,
        })
    }

    /// actions a player must always be able to reach on each device, to get
    /// around the Screens and out of a battle.
    fn required(self) -> bool {
        use Action::*;
        matches!(self, NavigateUp | NavigateDown | NavigateLeft | NavigateRight | Confirm | Back | Pause)
    }
}

/// a key, mouse button or controller button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    Key(KeyCode),
    Mouse(MouseButton),
    Pad(Button),
}

impl Source {
    pub fn device(self) -> Device {
        match self {
            Source::Key(_) | Source::Mouse(_) => Device::KeyboardMouse,
            Source::Pad(_) => Device::Controller,
        }
    }

    /// the input as a person would call it.
    pub fn name(self) -> String {
        match self {
            Source::Key(key) => crate::menus::key_name(key),
            Source::Mouse(MouseButton::Left) => "Left Click".to_owned(),
            Source::Mouse(MouseButton::Right) => "Right Click".to_owned(),
            Source::Mouse(MouseButton::Middle) => "Middle Click".to_owned(),
            Source::Mouse(MouseButton::Back) => "Mouse Back".to_owned(),
            Source::Mouse(MouseButton::Forward) => "Mouse Forward".to_owned(),
            Source::Mouse(MouseButton::Other(n)) => format!("Mouse {n}"),
            Source::Pad(button) => crate::gamepad::button_name(button).to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    KeyboardMouse,
    Controller,
}

/// what assigning an input already used by another action does to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolve {
    /// the other action takes this one's old input in that slot.
    Swap,
    /// the other action loses the input.
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindError {
    /// the slot is past the primary and secondary.
    NoSuchSlot,
    /// taking it would leave a Screen or battle unreachable from the device.
    Required(Action),
    /// the action takes no input from that device.
    NotOnDevice(Action),
}

/// each action's primary and secondary input, one map a device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, from = "StoredBindings")]
pub struct ActionBindings {
    pub keyboard_mouse: BTreeMap<Action, Vec<Source>>,
    pub controller: BTreeMap<Action, Vec<Source>>,
}

/// bindings as saved, which may predate some actions.
#[derive(Deserialize)]
#[serde(default)]
struct StoredBindings {
    keyboard_mouse: BTreeMap<Action, Vec<Source>>,
    controller: BTreeMap<Action, Vec<Source>>,
}

impl Default for StoredBindings {
    fn default() -> Self {
        let ActionBindings { keyboard_mouse, controller } = ActionBindings::default();
        StoredBindings { keyboard_mouse, controller }
    }
}

impl From<StoredBindings> for ActionBindings {
    /// an action saved bindings lack gets its defaults, less any input an
    /// action wanted at the same time already uses.
    fn from(stored: StoredBindings) -> Self {
        let mut bindings = ActionBindings { keyboard_mouse: stored.keyboard_mouse, controller: stored.controller };
        let defaults = ActionBindings::default();
        for device in [Device::KeyboardMouse, Device::Controller] {
            for (&action, sources) in defaults.map(device) {
                if bindings.map(device).contains_key(&action) { continue; }
                let free: Vec<Source> = sources.iter().copied().filter(|&source| bindings.conflicts(action, source).is_empty()).collect();
                bindings.map_mut(device).insert(action, free);
            }
        }
        bindings
    }
}

impl Default for ActionBindings {
    fn default() -> Self {
        use Action::*;
        let key = Source::Key;
        let pad = Source::Pad;
        let keyboard_mouse = [
            (MoveUp, vec![key(KeyCode::KeyW)]),
            (MoveDown, vec![key(KeyCode::KeyS)]),
            (MoveLeft, vec![key(KeyCode::KeyA)]),
            (MoveRight, vec![key(KeyCode::KeyD)]),
            (Walk, vec![key(KeyCode::ShiftLeft)]),
            (Attack, vec![Source::Mouse(MouseButton::Left), key(KeyCode::Space)]),
            (UsePower, vec![key(KeyCode::KeyE), Source::Mouse(MouseButton::Right)]),
            (PowerUp, vec![key(KeyCode::ArrowUp)]),
            (PowerDown, vec![key(KeyCode::ArrowDown)]),
            (PowerLeft, vec![key(KeyCode::ArrowLeft)]),
            (PowerRight, vec![key(KeyCode::ArrowRight)]),
            (VehicleAbility, vec![key(KeyCode::KeyF)]),
            (ExitVehicle, vec![key(KeyCode::KeyG)]),
            (ChangeView, vec![key(KeyCode::KeyV)]),
            (Pause, vec![key(KeyCode::Escape)]),
            (NavigateUp, vec![key(KeyCode::ArrowUp)]),
            (NavigateDown, vec![key(KeyCode::ArrowDown)]),
            (NavigateLeft, vec![key(KeyCode::ArrowLeft)]),
            (NavigateRight, vec![key(KeyCode::ArrowRight)]),
            (Confirm, vec![key(KeyCode::Enter), key(KeyCode::Space)]),
            (Back, vec![key(KeyCode::Escape), key(KeyCode::Backspace)]),
            (Skip, vec![key(KeyCode::KeyX)]),
            (PreviousIdol, vec![key(KeyCode::KeyQ)]),
            (NextIdol, vec![key(KeyCode::KeyE)]),
            (ZoomIn, vec![key(KeyCode::KeyR)]),
            (ZoomOut, vec![key(KeyCode::KeyF)]),
        ];
        let controller = [
            (Attack, vec![pad(Button::RightTrigger2)]),
            (UsePower, vec![pad(Button::RightTrigger)]),
            (PowerUp, vec![pad(Button::DPadUp)]),
            (PowerDown, vec![pad(Button::DPadDown)]),
            (PowerLeft, vec![pad(Button::DPadLeft)]),
            (PowerRight, vec![pad(Button::DPadRight)]),
            (VehicleAbility, vec![pad(Button::LeftThumb)]),
            (ExitVehicle, vec![pad(Button::RightThumb)]),
            (ChangeView, vec![pad(Button::North)]),
            (GyroAim, vec![pad(Button::LeftTrigger2)]),
            (Pause, vec![pad(Button::Start)]),
            (NavigateUp, vec![pad(Button::DPadUp)]),
            (NavigateDown, vec![pad(Button::DPadDown)]),
            (NavigateLeft, vec![pad(Button::DPadLeft)]),
            (NavigateRight, vec![pad(Button::DPadRight)]),
            (Confirm, vec![pad(Button::South)]),
            (Back, vec![pad(Button::East)]),
            (Skip, vec![pad(Button::Start)]),
            (PreviousIdol, vec![pad(Button::LeftTrigger)]),
            (NextIdol, vec![pad(Button::RightTrigger)]),
            (ZoomIn, vec![pad(Button::RightTrigger2)]),
            (ZoomOut, vec![pad(Button::LeftTrigger2)]),
        ];
        ActionBindings { keyboard_mouse: keyboard_mouse.into_iter().collect(), controller: controller.into_iter().collect() }
    }
}

impl ActionBindings {
    fn map(&self, device: Device) -> &BTreeMap<Action, Vec<Source>> {
        match device {
            Device::KeyboardMouse => &self.keyboard_mouse,
            Device::Controller => &self.controller,
        }
    }

    fn map_mut(&mut self, device: Device) -> &mut BTreeMap<Action, Vec<Source>> {
        match device {
            Device::KeyboardMouse => &mut self.keyboard_mouse,
            Device::Controller => &mut self.controller,
        }
    }

    /// the action's primary then secondary input on a device.
    pub fn sources(&self, device: Device, action: Action) -> &[Source] {
        self.map(device).get(&action).map_or(&[], Vec::as_slice)
    }

    /// the other actions wanted at the same time as action that already
    /// use source.
    pub fn conflicts(&self, action: Action, source: Source) -> Vec<Action> {
        self.map(source.device())
            .iter()
            .filter(|&(&other, sources)| other != action && other.overlaps(action) && sources.contains(&source))
            .map(|(&other, _)| other)
            .collect()
    }

    /// puts source in the action's slot (0 primary, 1 secondary), taking
    /// it from any action it conflicts with as resolve says.
    pub fn assign(&mut self, action: Action, slot: usize, source: Source, resolve: Resolve) -> Result<(), BindError> {
        if slot > 1 {
            return Err(BindError::NoSuchSlot);
        }
        let device = source.device();
        if !action.on(device) {
            return Err(BindError::NotOnDevice(action));
        }
        let previous = self.sources(device, action).get(slot).copied();
        let conflicts = self.conflicts(action, source);
        let mut next = self.clone();
        for other in conflicts {
            let sources = next.map_mut(device).entry(other).or_default();
            let at = sources.iter().position(|&bound| bound == source).expect("a conflict holds the source");
            match (resolve, previous) {
                (Resolve::Swap, Some(previous)) if !sources.contains(&previous) => sources[at] = previous,
                _ => {
                    sources.remove(at);
                }
            }
            if other.required() && sources.is_empty() {
                return Err(BindError::Required(other));
            }
        }
        let sources = next.map_mut(device).entry(action).or_default();
        // the action's own other slot holding it: the two change places
        match (sources.iter().position(|&bound| bound == source), sources.get(slot).copied()) {
            (Some(at), Some(previous)) => {
                sources[at] = previous;
                sources[slot] = source;
            }
            (Some(_), None) => {}
            (None, Some(_)) => sources[slot] = source,
            (None, None) => sources.push(source),
        }
        *self = next;
        Ok(())
    }

    /// takes the input out of the action's slot.
    pub fn clear(&mut self, device: Device, action: Action, slot: usize) -> Result<(), BindError> {
        let sources = self.map_mut(device).entry(action).or_default();
        if slot >= sources.len() {
            return Ok(());
        }
        if action.required() && sources.len() == 1 {
            return Err(BindError::Required(action));
        }
        sources.remove(slot);
        Ok(())
    }

    /// whether any action uses source.
    pub fn binds(&self, source: Source) -> bool {
        self.map(source.device()).values().any(|sources| sources.contains(&source))
    }

    /// the actions source starts in context.
    pub fn actions(&self, source: Source, context: Context) -> impl Iterator<Item = Action> + '_ {
        self.map(source.device())
            .iter()
            .filter(move |&(action, sources)| action.active(context) && sources.contains(&source))
            .map(|(&action, _)| action)
    }

    /// the console's button symbols as a device's inputs for the actions on
    /// those buttons, label drawing an input: A as Confirm's, B as Back's,
    /// L as Attack's, the +Control Pad as the menu directions' and the
    /// circle pad as Move's.
    pub fn prompts(&self, device: Device, label: impl Fn(Source) -> Label) -> Prompts {
        use Action::*;
        let mut prompts = Prompts::default();
        let first = |action: Action| self.sources(device, action).first().copied();
        for (input, action) in [
            (Input::A, Confirm),
            (Input::B, Back),
            (Input::L, Attack),
            (Input::X, ChangeView),
            (Input::Up, NavigateUp),
            (Input::Down, NavigateDown),
            (Input::Left, NavigateLeft),
            (Input::Right, NavigateRight),
        ] {
            if let Some(source) = first(action) {
                prompts.set(input, label(source));
            }
        }
        let directions = [NavigateUp, NavigateLeft, NavigateDown, NavigateRight].map(first);
        let moves = [MoveUp, MoveLeft, MoveDown, MoveRight].map(first);
        match device {
            Device::Controller => {
                let cross = [Button::DPadUp, Button::DPadLeft, Button::DPadDown, Button::DPadRight].map(|button| Some(Source::Pad(button)));
                if directions == cross {
                    prompts.set(Input::DPad, Label::Cross(None));
                    prompts.set(Input::UpDown, Label::Cross(Some(Direction::UpDown)));
                    prompts.set(Input::LeftRight, Label::Cross(Some(Direction::LeftRight)));
                }
                prompts.set(Input::CirclePad, Label::Stick("L".into()));
            }
            Device::KeyboardMouse => {
                // the keys side by side on one cap, as WASD reads
                let together = |sources: &[Option<Source>]| {
                    let names: Vec<String> = sources
                        .iter()
                        .flatten()
                        .map(|&source| match label(source) {
                            Label::Key(name) => name,
                            _ => source.name(),
                        })
                        .collect();
                    Label::Key(names.join(""))
                };
                prompts.set(Input::DPad, together(&directions));
                prompts.set(Input::UpDown, together(&[directions[0], directions[2]]));
                prompts.set(Input::LeftRight, together(&[directions[1], directions[3]]));
                prompts.set(Input::CirclePad, together(&moves));
            }
        }
        prompts
    }
}

/// the movement keys' lean shortened to Walk's, the length and not each
/// axis so a diagonal walks no faster.
pub fn walk(circle: [f32; 2], lean: f32) -> [f32; 2] {
    let length = circle[0].hypot(circle[1]);
    if length == 0.0 {
        return circle;
    }
    circle.map(|axis| axis * lean / length)
}

/// the console's input the held inputs make in a context: its buttons and
/// the circle pad and C-stick as keys push them.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Held {
    pub buttons: PadState,
    pub circle: [f32; 2],
    pub c_stick: [f32; 2],
    pub gyro_aim: bool,
    /// Walk is held: the circle pad is for walk to shorten.
    pub walk: bool,
}

/// the inputs held now, on both devices, each with the context it was
/// pressed in: an input acts only there, so Escape held from a battle into
/// the Pause it opened does not go on to close it as Back.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    held: Vec<(Source, Context)>,
}

impl Inputs {
    /// presses source in context, returning the actions the press starts:
    /// those it is bound to that no other held input already holds, so an
    /// action done once a press is done once however many of its inputs
    /// are down.
    pub fn press(&mut self, bindings: &ActionBindings, source: Source, context: Context) -> Vec<Action> {
        if self.held.iter().any(|&(held, _)| held == source) {
            return Vec::new();
        }
        let already = |action: Action| {
            self.held.iter().any(|&(held, pressed_in)| pressed_in == context && bindings.sources(held.device(), action).contains(&held))
        };
        let started = bindings.actions(source, context).filter(|&action| !already(action)).collect();
        self.held.push((source, context));
        started
    }

    pub fn release(&mut self, source: Source) {
        self.held.retain(|&(held, _)| held != source);
    }

    /// lets go of everything, when the window loses focus.
    pub fn release_all(&mut self) {
        self.held.clear();
    }

    /// lets go of the inputs of one kind, a controller's when it goes away
    /// or the mouse's when it stops aiming.
    pub fn release_where(&mut self, gone: impl Fn(Source) -> bool) {
        self.held.retain(|&(held, _)| !gone(held));
    }

    pub fn held(&self, bindings: &ActionBindings, context: Context) -> Held {
        let on = |action: Action| {
            action.active(context)
                && self.held.iter().any(|&(source, pressed_in)| {
                    pressed_in == context && bindings.sources(source.device(), action).contains(&source)
                })
        };
        let axis = |plus: Action, minus: Action| on(plus) as i8 as f32 - on(minus) as i8 as f32;
        let mut buttons = PadState::default();
        for action in Action::ALL {
            if let Some(button) = action.button().filter(|_| on(action)) {
                buttons.set(button, true);
            }
        }
        Held {
            buttons,
            circle: [axis(Action::MoveRight, Action::MoveLeft), axis(Action::MoveUp, Action::MoveDown)],
            c_stick: [axis(Action::AimRight, Action::AimLeft), axis(Action::AimUp, Action::AimDown)],
            gyro_aim: on(Action::GyroAim),
            walk: on(Action::Walk),
        }
    }
}

/// the Action Bindings an older settings file's console buttons make, as
/// far as each one has a single meaning; the rest keep their defaults.
pub fn migrate(keys: &crate::settings::Keys, pad: &crate::settings::PadButtons) -> ActionBindings {
    use crate::settings::{Keys, PadButtons};
    let mut bindings = ActionBindings::default();
    let (fresh_keys, fresh_pad) = (Keys::default(), PadButtons::default());
    let mut keep = |action: Action, old: Source, fresh: Source| {
        // only what the player changed; the console's defaults are not a
        // choice worth keeping over the new ones
        if old != fresh {
            let _ = bindings.assign(action, 0, old, Resolve::Swap);
        }
    };
    let key = Source::Key;
    for (action, old, fresh) in [
        (Action::MoveUp, keys.circle_up, fresh_keys.circle_up),
        (Action::MoveDown, keys.circle_down, fresh_keys.circle_down),
        (Action::MoveLeft, keys.circle_left, fresh_keys.circle_left),
        (Action::MoveRight, keys.circle_right, fresh_keys.circle_right),
        (Action::AimUp, keys.c_up, fresh_keys.c_up),
        (Action::AimDown, keys.c_down, fresh_keys.c_down),
        (Action::AimLeft, keys.c_left, fresh_keys.c_left),
        (Action::AimRight, keys.c_right, fresh_keys.c_right),
        (Action::Attack, keys.l, fresh_keys.l),
        (Action::UsePower, keys.power, fresh_keys.power),
        (Action::VehicleAbility, keys.vehicle_ability, fresh_keys.vehicle_ability),
        (Action::ExitVehicle, keys.vehicle_exit, fresh_keys.vehicle_exit),
        (Action::ChangeView, keys.x, fresh_keys.x),
        (Action::Confirm, keys.a, fresh_keys.a),
        (Action::Back, keys.b, fresh_keys.b),
    ] {
        keep(action, key(old), key(fresh));
    }
    let pad_source = Source::Pad;
    for (action, old, fresh) in [
        (Action::Confirm, pad.a, fresh_pad.a),
        (Action::Back, pad.b, fresh_pad.b),
        (Action::Attack, pad.l, fresh_pad.l),
        (Action::VehicleAbility, pad.vehicle_ability, fresh_pad.vehicle_ability),
        (Action::ExitVehicle, pad.vehicle_exit, fresh_pad.vehicle_exit),
        (Action::ChangeView, pad.x, fresh_pad.x),
    ] {
        keep(action, pad_source(old), pad_source(fresh));
    }
    bindings
}

/// the console buttons an older file changed that have no single action:
/// the D-pad, Start, Select, Y, R, ZL and ZR, for the player to review.
pub fn unmigrated(keys: &crate::settings::Keys, pad: &crate::settings::PadButtons) -> Vec<&'static str> {
    use crate::settings::{Keys, PadButtons};
    let mut keys_now = keys.clone();
    let mut keys_then = Keys::default();
    let mut changed: Vec<&'static str> = keys_now
        .all_mut()
        .into_iter()
        .zip(keys_then.all_mut())
        .filter(|((name, now), (_, then))| **now != **then && AMBIGUOUS.contains(name))
        .map(|((name, _), _)| name)
        .collect();
    let mut pad_now = pad.clone();
    let mut pad_then = PadButtons::default();
    for ((name, now), (_, then)) in pad_now.all_mut().into_iter().zip(pad_then.all_mut()) {
        if *now != *then && AMBIGUOUS.contains(&name) && !changed.contains(&name) {
            changed.push(name);
        }
    }
    changed
}

const AMBIGUOUS: [&str; 10] =
    ["Y", "R", "Start", "Select", "D-pad up", "D-pad down", "D-pad left", "D-pad right", "ZL", "ZR"];

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> Source {
        Source::Key(code)
    }

    /// two inputs on one action: letting go of one keeps it held.
    #[test]
    fn a_second_input_keeps_the_action_held() {
        let bindings = ActionBindings::default();
        let mut inputs = Inputs::default();
        inputs.press(&bindings, Source::Mouse(MouseButton::Left), Context::Gameplay);
        inputs.press(&bindings, key(KeyCode::Space), Context::Gameplay);
        inputs.release(key(KeyCode::Space));
        assert!(inputs.held(&bindings, Context::Gameplay).buttons.contains(PadState::L));
        inputs.release(Source::Mouse(MouseButton::Left));
        assert!(!inputs.held(&bindings, Context::Gameplay).buttons.contains(PadState::L));
    }

    /// Escape is Pause in a battle and Back in a Screen, and held from a
    /// battle into the Pause it opened it does not go on to press Back.
    #[test]
    fn one_key_does_its_contexts_action_only() {
        let bindings = ActionBindings::default();
        let mut inputs = Inputs::default();
        inputs.press(&bindings, key(KeyCode::Escape), Context::Gameplay);
        assert_eq!(inputs.held(&bindings, Context::Gameplay).buttons, PadState::START);
        assert_eq!(inputs.held(&bindings, Context::Screens).buttons, PadState::default());
        inputs.release(key(KeyCode::Escape));
        inputs.press(&bindings, key(KeyCode::Escape), Context::Screens);
        assert_eq!(inputs.held(&bindings, Context::Screens).buttons, PadState::B);
        // Space confirms in a Screen without firing, and fires in a battle
        let mut inputs = Inputs::default();
        inputs.press(&bindings, key(KeyCode::Space), Context::Screens);
        assert_eq!(inputs.held(&bindings, Context::Screens).buttons, PadState::A);
        let mut inputs = Inputs::default();
        inputs.press(&bindings, key(KeyCode::Space), Context::Gameplay);
        assert_eq!(inputs.held(&bindings, Context::Gameplay).buttons, PadState::L);
    }

    /// Change View is the game's X in a battle, as its own hint says.
    #[test]
    fn change_view_presses_x_in_a_battle() {
        let bindings = ActionBindings::default();
        let mut inputs = Inputs::default();
        inputs.press(&bindings, key(KeyCode::KeyV), Context::Gameplay);
        assert_eq!(inputs.held(&bindings, Context::Gameplay).buttons, PadState::X);
        let pad = bindings.prompts(Device::Controller, |source| Label::Key(source.name()));
        assert_eq!(pad.get(Input::X), &Label::Key("Top face".into()));
    }

    /// movement keys push the circle pad; opposite keys cancel out.
    #[test]
    fn movement_keys_push_the_circle_pad() {
        let bindings = ActionBindings::default();
        let mut inputs = Inputs::default();
        inputs.press(&bindings, key(KeyCode::KeyW), Context::Gameplay);
        inputs.press(&bindings, key(KeyCode::KeyA), Context::Gameplay);
        assert_eq!(inputs.held(&bindings, Context::Gameplay).circle, [-1.0, 1.0]);
        inputs.press(&bindings, key(KeyCode::KeyD), Context::Gameplay);
        assert_eq!(inputs.held(&bindings, Context::Gameplay).circle, [0.0, 1.0]);
        let mut inputs = Inputs::default();
        inputs.press(&bindings, key(KeyCode::KeyS), Context::Screens);
        assert_eq!(inputs.held(&bindings, Context::Screens).circle, [0.0, -1.0]);
    }

    /// Walk is held in a battle only, and shortens a lean to its length,
    /// diagonals included, keeping no lean none.
    #[test]
    fn walk_shortens_the_lean() {
        let bindings = ActionBindings::default();
        let mut inputs = Inputs::default();
        inputs.press(&bindings, key(KeyCode::ShiftLeft), Context::Gameplay);
        assert!(inputs.held(&bindings, Context::Gameplay).walk);
        inputs.release(key(KeyCode::ShiftLeft));
        assert!(!inputs.held(&bindings, Context::Gameplay).walk);
        inputs.press(&bindings, key(KeyCode::ShiftLeft), Context::Screens);
        assert!(!inputs.held(&bindings, Context::Screens).walk, "a Screen's movement is never shortened");

        assert_eq!(walk([0.0, 0.0], 0.5), [0.0, 0.0], "Walk alone does not move");
        assert_eq!(walk([0.0, 1.0], 0.5), [0.0, 0.5]);
        let [x, y] = walk([1.0, 1.0], 0.5);
        assert!((x.hypot(y) - 0.5).abs() < 1e-6 && (x - y).abs() < 1e-6, "diagonal {x}, {y}");
    }

    /// Walk is the keyboard's: no controller input can be given to it, and
    /// a saved file from before it gets Shift.
    #[test]
    fn walk_is_keyboard_only() {
        let mut bindings = ActionBindings::default();
        assert_eq!(bindings.sources(Device::Controller, Action::Walk), &[]);
        assert_eq!(
            bindings.assign(Action::Walk, 0, Source::Pad(Button::LeftThumb), Resolve::Replace),
            Err(BindError::NotOnDevice(Action::Walk))
        );
        bindings.assign(Action::Walk, 0, key(KeyCode::KeyC), Resolve::Replace).unwrap();
        assert_eq!(bindings.sources(Device::KeyboardMouse, Action::Walk), &[key(KeyCode::KeyC)]);
        let saved = "[keyboard_mouse]\nmove_up = [{ Key = \"KeyW\" }]\n";
        let bindings: ActionBindings = toml::from_str(saved).unwrap();
        assert_eq!(bindings.sources(Device::KeyboardMouse, Action::Walk), &[key(KeyCode::ShiftLeft)]);
    }

    /// an input used in the same context is a conflict; one used only in
    /// the other context is not.
    #[test]
    fn saved_bindings_without_the_idol_actions_get_their_free_defaults() {
        // Q taken by Confirm in the saved map: Previous Idol gets no key
        let saved = "[keyboard_mouse]\nconfirm = [{ Key = \"KeyQ\" }]\n";
        let bindings: ActionBindings = toml::from_str(saved).unwrap();
        assert_eq!(bindings.sources(Device::KeyboardMouse, Action::PreviousIdol), &[]);
        assert_eq!(bindings.sources(Device::KeyboardMouse, Action::NextIdol), &[key(KeyCode::KeyE)]);
        assert_eq!(bindings.sources(Device::Controller, Action::ZoomIn), &[Source::Pad(Button::RightTrigger2)]);
    }

    #[test]
    fn conflicts_are_only_within_a_context() {
        let bindings = ActionBindings::default();
        // G is Exit Vehicle, a battle's; Confirm is a Screen's
        assert!(bindings.conflicts(Action::Confirm, key(KeyCode::KeyG)).is_empty());
        // W moves everywhere, so it conflicts with both kinds
        assert_eq!(bindings.conflicts(Action::Attack, key(KeyCode::KeyW)), vec![Action::MoveUp]);
        assert_eq!(bindings.conflicts(Action::Confirm, key(KeyCode::KeyW)), vec![Action::MoveUp]);
    }

    /// Swap gives the other action the old input; Replace takes it away.
    #[test]
    fn swap_and_replace() {
        let mut bindings = ActionBindings::default();
        bindings.assign(Action::Attack, 0, key(KeyCode::KeyE), Resolve::Swap).unwrap();
        assert_eq!(bindings.sources(Device::KeyboardMouse, Action::Attack)[0], key(KeyCode::KeyE));
        assert_eq!(bindings.sources(Device::KeyboardMouse, Action::UsePower)[0], Source::Mouse(MouseButton::Left));

        let mut bindings = ActionBindings::default();
        bindings.assign(Action::Attack, 0, key(KeyCode::KeyE), Resolve::Replace).unwrap();
        assert_eq!(bindings.sources(Device::KeyboardMouse, Action::UsePower), &[Source::Mouse(MouseButton::Right)]);
        assert_eq!(bindings.sources(Device::KeyboardMouse, Action::Attack), &[key(KeyCode::KeyE), key(KeyCode::Space)]);
    }

    /// an action's secondary input put in its primary slot swaps the two,
    /// losing neither.
    #[test]
    fn moving_an_input_within_an_action_swaps_its_slots() {
        let mut bindings = ActionBindings::default();
        bindings.assign(Action::Attack, 0, key(KeyCode::Space), Resolve::Replace).unwrap();
        assert_eq!(bindings.sources(Device::KeyboardMouse, Action::Attack), &[key(KeyCode::Space), Source::Mouse(MouseButton::Left)]);
    }

    /// a press starts an action only when none of its other inputs holds it,
    /// so Use Power on both of its inputs uses one power.
    #[test]
    fn an_action_starts_once_however_many_inputs_press_it() {
        let bindings = ActionBindings::default();
        let mut inputs = Inputs::default();
        assert_eq!(inputs.press(&bindings, key(KeyCode::KeyE), Context::Gameplay), vec![Action::UsePower]);
        assert!(inputs.press(&bindings, Source::Mouse(MouseButton::Right), Context::Gameplay).is_empty());
        assert!(inputs.press(&bindings, key(KeyCode::KeyE), Context::Gameplay).is_empty(), "a held input is not pressed again");
        inputs.release(key(KeyCode::KeyE));
        inputs.release(Source::Mouse(MouseButton::Right));
        assert_eq!(inputs.press(&bindings, Source::Mouse(MouseButton::Right), Context::Gameplay), vec![Action::UsePower]);
        // the mouse stops aiming: its inputs let go, the keyboard's stay
        inputs.press(&bindings, key(KeyCode::Space), Context::Gameplay);
        inputs.release_where(|source| matches!(source, Source::Mouse(_)));
        assert!(inputs.held(&bindings, Context::Gameplay).buttons.contains(PadState::L));
        assert!(inputs.press(&bindings, Source::Mouse(MouseButton::Right), Context::Gameplay) == vec![Action::UsePower]);
    }

    /// the game's prompts show the inputs on the actions: A as Confirm's,
    /// following a rebinding, and the circle pad as the movement keys.
    #[test]
    fn prompts_follow_the_action_bindings() {
        let keycap = |source: Source| Label::Key(source.name());
        let mut bindings = ActionBindings::default();
        let prompts = bindings.prompts(Device::KeyboardMouse, keycap);
        assert_eq!(prompts.get(Input::A), &Label::Key("Enter".into()));
        assert_eq!(prompts.get(Input::CirclePad), &Label::Key("WASD".into()));
        bindings.assign(Action::Confirm, 0, key(KeyCode::KeyJ), Resolve::Replace).unwrap();
        assert_eq!(bindings.prompts(Device::KeyboardMouse, keycap).get(Input::A), &Label::Key("J".into()));
        let pad = bindings.prompts(Device::Controller, |_| Label::Round("?".into()));
        assert_eq!(pad.get(Input::DPad), &Label::Cross(None), "the menu directions on the d-pad show as it");
    }

    /// Back, Confirm, the menu directions and Pause always keep an input.
    #[test]
    fn required_actions_keep_an_input() {
        let mut bindings = ActionBindings::default();
        bindings.clear(Device::KeyboardMouse, Action::Back, 1).unwrap();
        assert_eq!(bindings.clear(Device::KeyboardMouse, Action::Back, 0), Err(BindError::Required(Action::Back)));
        let before = bindings.clone();
        // taking Pause's only key for a battle action is refused whole
        assert_eq!(
            bindings.assign(Action::Attack, 0, key(KeyCode::Escape), Resolve::Replace),
            Err(BindError::Required(Action::Pause))
        );
        assert_eq!(bindings, before);
        // a Swap gives Pause the old input, so it is allowed
        bindings.assign(Action::Attack, 0, key(KeyCode::Escape), Resolve::Swap).unwrap();
        assert_eq!(bindings.sources(Device::KeyboardMouse, Action::Pause), &[Source::Mouse(MouseButton::Left)]);
    }

    /// the bindings are kept between runs.
    #[test]
    fn bindings_come_back_as_they_were_saved() {
        let mut bindings = ActionBindings::default();
        bindings.assign(Action::Attack, 1, Source::Mouse(MouseButton::Back), Resolve::Replace).unwrap();
        bindings.assign(Action::UsePower, 0, Source::Pad(Button::West), Resolve::Replace).unwrap();
        let text = toml::to_string_pretty(&bindings).unwrap();
        assert_eq!(toml::from_str::<ActionBindings>(&text).unwrap(), bindings);
    }

    /// an older file's changed buttons with one meaning carry over; ones
    /// without are listed for review.
    #[test]
    fn older_bindings_migrate_where_unambiguous() {
        use crate::settings::{Keys, PadButtons};
        let keys = Keys { l: KeyCode::KeyK, circle_up: KeyCode::KeyI, x: KeyCode::KeyC, r: KeyCode::KeyV, ..Keys::default() };
        let pad = PadButtons { a: Button::South, b: Button::East, ..PadButtons::default() };
        let bindings = migrate(&keys, &pad);
        assert_eq!(bindings.sources(Device::KeyboardMouse, Action::Attack)[0], key(KeyCode::KeyK));
        // I was the console's default, so the new default W stays
        assert_eq!(bindings.sources(Device::KeyboardMouse, Action::MoveUp)[0], key(KeyCode::KeyW));
        assert_eq!(bindings.sources(Device::Controller, Action::Confirm), &[Source::Pad(Button::South)]);
        assert_eq!(bindings.sources(Device::Controller, Action::Back), &[Source::Pad(Button::East)]);
        assert_eq!(bindings.sources(Device::KeyboardMouse, Action::ChangeView)[0], key(KeyCode::KeyC));
        assert_eq!(unmigrated(&keys, &pad), vec!["R"]);
    }
}
