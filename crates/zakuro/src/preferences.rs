//! the PC Preferences of Kid Icarus: Uprising, the choices its own Options
//! Screens show and change: aiming, the controller's sticks and gyro, and
//! the presentation of the top screen. kept with the other settings, apart
//! from the game's own save books.

use serde::{Deserialize, Serialize};

use crate::bindings::ActionBindings;
use crate::settings::Settings;

/// when the gyro aims, if ever.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GyroMode {
    #[default]
    Off,
    /// all through gameplay, never in a Screen.
    Always,
    /// while its activation is held.
    Hold,
}

/// aiming with the mouse, the right stick and the gyro. vertical inversion
/// is the game's own (Reticle/Camera's Invert Vertical Movement), not here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Aim {
    /// reticle pixels a count of the mouse moves it.
    pub mouse_sensitivity: f32,
    /// how fast a mouse drag Orbits the Idol View, 1 for 0.3° a window pixel.
    pub idol_view_sensitivity: f32,
    /// how far the right stick turns, 1 as the game's own C-stick does.
    pub stick_sensitivity: f32,
    /// how far either stick leans before it counts, 0 to 1.
    pub dead_zone: f32,
    pub gyro: GyroMode,
    /// how far a turn of the controller turns, 1 as the game's own gyro.
    pub gyro_sensitivity: f32,
    pub gyro_invert_x: bool,
    pub gyro_invert_y: bool,
}

impl Default for Aim {
    fn default() -> Self {
        Aim {
            mouse_sensitivity: 0.5,
            idol_view_sensitivity: 1.0,
            stick_sensitivity: 1.0,
            dead_zone: 0.15,
            gyro: GyroMode::Off,
            gyro_sensitivity: 1.0,
            gyro_invert_x: false,
            gyro_invert_y: false,
        }
    }
}

/// how the top screen shows the game's HUD and portraits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Presentation {
    /// the HUD and reticle, 1 as the game draws them.
    pub hud_scale: f32,
    /// the portraits on the top screen, 0 for none.
    pub portrait_scale: f32,
}

impl Default for Presentation {
    fn default() -> Self {
        Presentation { hud_scale: 0.7, portrait_scale: 0.5 }
    }
}

impl Presentation {
    /// the smallest and largest HUD the top screen draws legibly.
    pub const HUD: (f32, f32) = (0.5, 1.0);
    pub const PORTRAIT: (f32, f32) = (0.0, 1.0);
}

/// moving Pit with the keyboard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Movement {
    /// how far the movement keys lean the circle pad while Walk is held.
    /// Pit, measured on a ground battle: 0.5 settles at about a third of
    /// his running speed; about 0.86 or more, reached within five frames
    /// of standing, dashes and then sprints, which a key's instant full
    /// lean always does. Walk to full lean runs without dashing.
    pub walk_lean: f32,
    /// the movement keys walk by themselves, and Walk held gives them the
    /// full lean, so it runs.
    pub walk_by_default: bool,
}

impl Default for Movement {
    fn default() -> Self {
        Movement { walk_lean: 0.5, walk_by_default: false }
    }
}

impl Movement {
    /// below 0.3 the console's dead zone keeps Pit still; above 0.8 he
    /// comes near dashing.
    pub const WALK_LEAN: (f32, f32) = (0.3, 0.8);

    /// the lean Walk gives, within WALK_LEAN whatever the file says.
    pub fn walk_lean(&self) -> f32 {
        self.walk_lean.clamp(Self::WALK_LEAN.0, Self::WALK_LEAN.1)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PcPreferences {
    pub aim: Aim,
    pub presentation: Presentation,
    pub movement: Movement,
    pub bindings: ActionBindings,
    /// the console buttons an older settings file changed that have no
    /// single action, kept until the player has seen them.
    pub review: Vec<String>,
}

/// the groups of choices Restore Defaults puts back one at a time, each
/// the Screen that shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    KeyboardMouse,
    Controller,
    ReticleCamera,
    Presentation,
}

impl Category {
    /// puts this category's choices back as they come, leaving the others
    /// and everything outside the PC Preferences as they are.
    pub fn restore_defaults(self, settings: &mut Settings) {
        let defaults = Settings::default();
        match self {
            Category::KeyboardMouse => {
                settings.pc.bindings.keyboard_mouse = defaults.pc.bindings.keyboard_mouse;
                let aim = &mut settings.pc.aim;
                aim.mouse_sensitivity = defaults.pc.aim.mouse_sensitivity;
                aim.idol_view_sensitivity = defaults.pc.aim.idol_view_sensitivity;
                settings.pc.movement = defaults.pc.movement;
            }
            Category::Controller => {
                settings.pc.bindings.controller = defaults.pc.bindings.controller;
                let (aim, fresh) = (&mut settings.pc.aim, &defaults.pc.aim);
                aim.stick_sensitivity = fresh.stick_sensitivity;
                aim.dead_zone = fresh.dead_zone;
            }
            Category::ReticleCamera => {
                let (aim, fresh) = (&mut settings.pc.aim, defaults.pc.aim);
                // the mouse's and sticks' own choices are on their Screens
                *aim = Aim {
                    mouse_sensitivity: aim.mouse_sensitivity,
                    idol_view_sensitivity: aim.idol_view_sensitivity,
                    stick_sensitivity: aim.stick_sensitivity,
                    dead_zone: aim.dead_zone,
                    ..fresh
                };
            }
            Category::Presentation => settings.pc.presentation = defaults.pc.presentation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::{Action, Device, Resolve, Source};
    use winit::keyboard::KeyCode;

    fn customised() -> Settings {
        let mut settings = Settings::default();
        settings.keys.a = KeyCode::KeyK;
        let bindings = &mut settings.pc.bindings;
        bindings.assign(Action::Attack, 0, Source::Key(KeyCode::KeyK), Resolve::Replace).unwrap();
        bindings.assign(Action::Attack, 0, Source::Pad(gilrs::Button::West), Resolve::Replace).unwrap();
        settings.volume = 0.3;
        settings.scale = 4;
        settings.pc.aim = Aim {
            mouse_sensitivity: 2.0,
            stick_sensitivity: 1.5,
            dead_zone: 0.3,
            gyro: GyroMode::Hold,
            gyro_sensitivity: 2.0,
            ..Aim::default()
        };
        settings.pc.presentation.hud_scale = 0.9;
        settings.pc.movement.walk_lean = 0.7;
        settings.pc.movement.walk_by_default = true;
        settings
    }

    /// Restore Defaults on Presentation puts its scales back and nothing else.
    #[test]
    fn restoring_presentation_keeps_the_other_categories() {
        let mut settings = customised();
        Category::Presentation.restore_defaults(&mut settings);
        assert_eq!(settings.pc.presentation.hud_scale, 0.7);
        let mut expected = customised();
        expected.pc.presentation = Presentation::default();
        assert_eq!(settings, expected);
    }

    /// the keyboard's reset leaves the controller's bindings and sticks.
    #[test]
    fn restoring_one_device_keeps_the_other() {
        let attack = |settings: &Settings, device| settings.pc.bindings.sources(device, Action::Attack)[0];
        let mut settings = customised();
        Category::KeyboardMouse.restore_defaults(&mut settings);
        assert_eq!(attack(&settings, Device::KeyboardMouse), Source::Mouse(winit::event::MouseButton::Left));
        assert_eq!(settings.pc.aim.mouse_sensitivity, 0.5);
        assert_eq!(settings.pc.movement, Movement::default(), "Walk speed is the keyboard's");
        assert_eq!(attack(&settings, Device::Controller), Source::Pad(gilrs::Button::West));
        assert_eq!(settings.pc.aim.dead_zone, 0.3);
        assert_eq!(settings.keys.a, KeyCode::KeyK, "other titles' console keys are not the PC Preferences");

        let mut settings = customised();
        Category::Controller.restore_defaults(&mut settings);
        assert_eq!(attack(&settings, Device::Controller), Source::Pad(gilrs::Button::RightTrigger2));
        assert_eq!(settings.pc.aim.dead_zone, 0.15);
        assert_eq!(attack(&settings, Device::KeyboardMouse), Source::Key(KeyCode::KeyK));
        assert_eq!(settings.pc.aim.mouse_sensitivity, 2.0);
        assert_eq!(settings.pc.aim.gyro, GyroMode::Hold, "the gyro is Reticle/Camera's");
        assert_eq!(settings.pc.movement.walk_lean, 0.7);
    }

    /// Walk's lean is kept, comes as half from a file without it, and stays
    /// short of a dash and past the dead zone whatever a file says.
    #[test]
    fn walk_speed_is_kept_and_bounded() {
        let settings = Settings::parse("volume = 0.4\n").unwrap();
        assert_eq!(settings.pc.movement.walk_lean(), 0.5);
        assert!(!settings.pc.movement.walk_by_default);
        let mut changed = settings.clone();
        changed.pc.movement.walk_lean = 0.65;
        changed.pc.movement.walk_by_default = true;
        let back = Settings::parse(&toml::to_string_pretty(&changed).unwrap()).unwrap();
        assert_eq!(back.pc.movement.walk_lean(), 0.65);
        assert!(back.pc.movement.walk_by_default);
        for (saved, given) in [(1.0, 0.8), (0.0, 0.3)] {
            assert_eq!(Movement { walk_lean: saved, ..Movement::default() }.walk_lean(), given);
        }
    }

    /// Reticle/Camera resets the gyro, not the mouse's or sticks' feel.
    #[test]
    fn restoring_reticle_camera_resets_the_gyro_only() {
        let mut settings = customised();
        Category::ReticleCamera.restore_defaults(&mut settings);
        assert_eq!(settings.pc.aim.gyro, GyroMode::Off);
        assert_eq!(settings.pc.aim.gyro_sensitivity, 1.0);
        assert_eq!(settings.pc.aim.mouse_sensitivity, 2.0);
        assert_eq!(settings.pc.aim.stick_sensitivity, 1.5);
        assert_eq!(settings.volume, 0.3);
    }

    /// a settings file from before the PC Preferences keeps its choices,
    /// its changed console keys carried over as actions where they have one
    /// meaning and listed for review where not.
    #[test]
    fn an_older_settings_file_keeps_its_choices() {
        let text = "volume = 0.4\n[keys]\nl = \"KeyK\"\nr = \"KeyV\"\n[pad]\na = \"South\"\nb = \"East\"\n";
        let settings = Settings::parse(text).unwrap();
        assert_eq!(settings.keys.l, KeyCode::KeyK);
        assert_eq!(settings.volume, 0.4);
        let bindings = &settings.pc.bindings;
        assert_eq!(bindings.sources(Device::KeyboardMouse, Action::Attack)[0], Source::Key(KeyCode::KeyK));
        assert_eq!(bindings.sources(Device::Controller, Action::Confirm), &[Source::Pad(gilrs::Button::South)]);
        assert_eq!(settings.pc.review, vec!["R".to_owned()]);
        assert_eq!(settings.pc.presentation, Presentation::default());

        // saved, the notice waits for the player; a load does not migrate
        // again
        let saved = toml::to_string_pretty(&settings).unwrap();
        let mut changed = Settings::parse(&saved).unwrap();
        assert_eq!(changed.pc.review, vec!["R".to_owned()]);
        changed.pc.review.clear();
        changed.pc.bindings.assign(Action::Attack, 0, Source::Key(KeyCode::KeyZ), Resolve::Replace).unwrap();
        let back = Settings::parse(&toml::to_string_pretty(&changed).unwrap()).unwrap();
        assert_eq!(back.pc.bindings.sources(Device::KeyboardMouse, Action::Attack)[0], Source::Key(KeyCode::KeyZ));
    }
}
