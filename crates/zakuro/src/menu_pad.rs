//! a controller moving around the Frontend Menu and the windows it opens:
//! the d-pad or the left stick moves the focus, held it repeats, and the
//! player's Confirm and Back buttons press the focused widget and close
//! the window on top.

use std::time::{Duration, Instant};

use gilrs::Button;

/// how long a direction is held before it repeats.
const REPEAT_DELAY: Duration = Duration::from_millis(400);
/// how often a held direction repeats after that.
const REPEAT_EVERY: Duration = Duration::from_millis(110);
/// how far the left stick leans before it counts as a direction.
const STICK_LEAN: f32 = 0.6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    fn of_dpad(button: Button) -> Option<Direction> {
        match button {
            Button::DPadUp => Some(Direction::Up),
            Button::DPadDown => Some(Direction::Down),
            Button::DPadLeft => Some(Direction::Left),
            Button::DPadRight => Some(Direction::Right),
            _ => None,
        }
    }

    /// the way a stick leans furthest, up positive, once it leans far enough.
    fn of_stick([x, y]: [f32; 2]) -> Option<Direction> {
        if x.hypot(y) < STICK_LEAN {
            None
        } else if x.abs() > y.abs() {
            Some(if x > 0.0 { Direction::Right } else { Direction::Left })
        } else {
            Some(if y > 0.0 { Direction::Up } else { Direction::Down })
        }
    }

    pub fn key(self) -> egui::Key {
        match self {
            Direction::Up => egui::Key::ArrowUp,
            Direction::Down => egui::Key::ArrowDown,
            Direction::Left => egui::Key::ArrowLeft,
            Direction::Right => egui::Key::ArrowRight,
        }
    }
}

/// what a controller asks of the menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Nav {
    Move(Direction),
    Confirm,
    Back,
}

#[derive(Default)]
pub struct MenuPad {
    /// the d-pad directions held, the last pressed last.
    dpad: Vec<Direction>,
    /// the direction moving the focus, and when it next repeats.
    held: Option<(Direction, Instant)>,
}

impl MenuPad {
    /// a controller button pressed or let go: Confirm and Back answer at
    /// once, a d-pad direction waits for the tick.
    pub fn button(&mut self, button: Button, pressed: bool, confirm: &[Button], back: &[Button]) -> Option<Nav> {
        if let Some(direction) = Direction::of_dpad(button) {
            self.dpad.retain(|&held| held != direction);
            if pressed {
                self.dpad.push(direction);
            }
            return None;
        }
        if !pressed {
            None
        } else if confirm.contains(&button) {
            Some(Nav::Confirm)
        } else if back.contains(&button) {
            Some(Nav::Back)
        } else {
            None
        }
    }

    /// the focus moves this frame, if any: once as a direction starts, then
    /// again while it's held.
    pub fn tick(&mut self, stick: [f32; 2], now: Instant) -> Option<Nav> {
        let Some(direction) = self.dpad.last().copied().or_else(|| Direction::of_stick(stick)) else {
            self.held = None;
            return None;
        };
        match self.held {
            Some((held, next)) if held == direction => {
                if now < next {
                    return None;
                }
                self.held = Some((direction, now + REPEAT_EVERY));
            }
            _ => self.held = Some((direction, now + REPEAT_DELAY)),
        }
        Some(Nav::Move(direction))
    }

    /// lets go of everything, as the menu closes or a controller goes away.
    pub fn release(&mut self) {
        self.dpad.clear();
        self.held = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIRM: &[Button] = &[Button::South];
    const BACK: &[Button] = &[Button::East];

    #[test]
    fn a_direction_moves_once_then_repeats_while_held() {
        let mut pad = MenuPad::default();
        let start = Instant::now();
        pad.button(Button::DPadDown, true, CONFIRM, BACK);
        assert_eq!(pad.tick([0.0; 2], start), Some(Nav::Move(Direction::Down)));
        assert_eq!(pad.tick([0.0; 2], start + Duration::from_millis(100)), None);
        assert_eq!(pad.tick([0.0; 2], start + REPEAT_DELAY), Some(Nav::Move(Direction::Down)));
        assert_eq!(pad.tick([0.0; 2], start + REPEAT_DELAY + Duration::from_millis(50)), None);
        assert_eq!(pad.tick([0.0; 2], start + REPEAT_DELAY + REPEAT_EVERY), Some(Nav::Move(Direction::Down)));
        pad.button(Button::DPadDown, false, CONFIRM, BACK);
        assert_eq!(pad.tick([0.0; 2], start + REPEAT_DELAY * 3), None);
    }

    #[test]
    fn the_stick_moves_the_way_it_leans_furthest() {
        let mut pad = MenuPad::default();
        let now = Instant::now();
        assert_eq!(pad.tick([0.3, 0.2], now), None);
        assert_eq!(pad.tick([0.8, -0.3], now), Some(Nav::Move(Direction::Right)));
        assert_eq!(pad.tick([0.2, -0.9], now), Some(Nav::Move(Direction::Down)));
    }

    #[test]
    fn the_last_dpad_direction_pressed_wins_over_the_stick() {
        let mut pad = MenuPad::default();
        let now = Instant::now();
        pad.button(Button::DPadUp, true, CONFIRM, BACK);
        pad.button(Button::DPadLeft, true, CONFIRM, BACK);
        assert_eq!(pad.tick([0.0, -1.0], now), Some(Nav::Move(Direction::Left)));
        pad.button(Button::DPadLeft, false, CONFIRM, BACK);
        assert_eq!(pad.tick([0.0, -1.0], now), Some(Nav::Move(Direction::Up)));
    }

    #[test]
    fn confirm_and_back_answer_as_pressed_only() {
        let mut pad = MenuPad::default();
        assert_eq!(pad.button(Button::South, true, CONFIRM, BACK), Some(Nav::Confirm));
        assert_eq!(pad.button(Button::South, false, CONFIRM, BACK), None);
        assert_eq!(pad.button(Button::East, true, CONFIRM, BACK), Some(Nav::Back));
        assert_eq!(pad.button(Button::North, true, CONFIRM, BACK), None);
    }
}
