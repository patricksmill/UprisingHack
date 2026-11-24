//! Bindings to try Dynamic Input Textures with: keyboard A E, B Esc, X V,
//! Y Q, L Left Click, R Right Click, D-Pad 1234, circle pad WASD; or a
//! PlayStation controller's buttons; and the Idols actions (Q/E, R/F, the
//! mouse's drags, or L1/R1, R2/L2, the right stick).

use zakuro_core::services::prompts::{Host, Input, Label, MouseButton, Prompts};

pub fn sample(controller: bool) -> Prompts {
    let mut prompts = Prompts::default();
    if controller {
        prompts.set_host(Host::Controller);
        for (input, label) in [
            (Input::A, Label::Round("✕".into())),
            (Input::B, Label::Round("○".into())),
            (Input::X, Label::Round("△".into())),
            (Input::Y, Label::Round("□".into())),
            (Input::L, Label::Shoulder("L2".into())),
            (Input::R, Label::Shoulder("R2".into())),
            (Input::CirclePad, Label::Stick("L".into())),
        ] {
            prompts.set(input, label);
        }
    } else {
        prompts.set_host(Host::KeyboardMouse);
        for (input, label) in [
            (Input::A, Label::Key("E".into())),
            (Input::B, Label::Key("Esc".into())),
            (Input::X, Label::Key("V".into())),
            (Input::Y, Label::Key("Q".into())),
            (Input::L, Label::Mouse(MouseButton::Left)),
            (Input::R, Label::Mouse(MouseButton::Right)),
            (Input::DPad, Label::Key("1234".into())),
            (Input::CirclePad, Label::Key("WASD".into())),
        ] {
            prompts.set(input, label);
        }
    }
    // Kid Icarus' Idols actions, as the frontend names them
    let actions: [(&str, Label); 6] = if controller {
        [
            ("PreviousIdol", Label::Shoulder("L1".into())),
            ("NextIdol", Label::Shoulder("R1".into())),
            ("ZoomIn", Label::Shoulder("R2".into())),
            ("ZoomOut", Label::Shoulder("L2".into())),
            ("IdolViewOrbit", Label::Stick("R".into())),
            ("IdolViewPan", Label::Cross(None)),
        ]
    } else {
        [
            ("PreviousIdol", Label::Key("Q".into())),
            ("NextIdol", Label::Key("E".into())),
            ("ZoomIn", Label::Key("R".into())),
            ("ZoomOut", Label::Key("F".into())),
            ("IdolViewOrbit", Label::Mouse(MouseButton::Left)),
            ("IdolViewPan", Label::Mouse(MouseButton::Right)),
        ]
    };
    for (name, label) in actions {
        prompts.set_action(name, Some(label));
    }
    prompts
}
