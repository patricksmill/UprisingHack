//! a controller's gyroscope, read through SDL, which knows the motion
//! sensors of DualShock 4, DualSense and Switch controllers. gilrs reads
//! the buttons and sticks; this reads only the turning, given to the
//! console's own gyroscope for the titles and patches that aim with it.

use std::time::{Duration, Instant};

use sdl3::sensor::SensorType;

use crate::preferences::{Aim, GyroMode};

/// how long a calibration averages a controller lying still.
const CALIBRATION: Duration = Duration::from_secs(1);

/// how often to look again for a controller with a gyroscope, one being
/// plugged in or out.
const RESCAN: Duration = Duration::from_secs(2);

/// what the gyro can do now, to tell the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// SDL could not start, so no gyroscope can be read.
    Unavailable,
    NoController,
    /// controllers, none with a gyroscope.
    NoGyroscope,
    Calibrating,
    Ready,
}

pub struct Gyro {
    /// SDL and its gamepads, none when it could not start.
    gamepads: Option<(sdl3::Sdl, sdl3::GamepadSubsystem)>,
    /// the open controller with a gyroscope, and its name.
    pad: Option<(sdl3::gamepad::Gamepad, String)>,
    any_controller: bool,
    scanned: Option<Instant>,
    /// what the still controller reads, taken out of every reading.
    bias: [f32; 3],
    /// a calibration under way: when it started, and the readings so far.
    calibrating: Option<(Instant, [f32; 3], u32)>,
    /// the last reading, its rest taken out, degrees a second.
    reading: [f32; 3],
}

impl Gyro {
    pub fn new() -> Gyro {
        let gamepads = sdl3::init()
            .and_then(|sdl| sdl.gamepad().map(|gamepads| (sdl, gamepads)))
            .inspect_err(|error| log::warn!("controller gyroscopes are not available, {error}"))
            .ok();
        Gyro { gamepads, pad: None, any_controller: false, scanned: None, bias: [0.0; 3], calibrating: None, reading: [0.0; 3] }
    }

    pub fn status(&self) -> Status {
        match (&self.gamepads, &self.pad, self.calibrating) {
            (None, ..) => Status::Unavailable,
            (_, Some(_), Some(_)) => Status::Calibrating,
            (_, Some(_), None) => Status::Ready,
            (_, None, _) if self.any_controller => Status::NoGyroscope,
            _ => Status::NoController,
        }
    }

    /// the controller being read, by name.
    pub fn controller(&self) -> Option<&str> {
        self.pad.as_ref().map(|(_, name)| name.as_str())
    }

    /// starts taking what the controller reads lying still as its rest.
    pub fn calibrate(&mut self) {
        self.calibrating = Some((Instant::now(), [0.0; 3], 0));
    }

    /// opens a controller with a gyroscope when none is, and lets go of
    /// one that went away.
    fn scan(&mut self, now: Instant) {
        let Some((_, gamepads)) = &self.gamepads else { return };
        gamepads.update();
        if self.pad.as_ref().is_some_and(|(pad, _)| !pad.connected()) {
            log::info!("the gyroscope's controller went away");
            self.pad = None;
            self.calibrating = None;
        }
        if self.pad.is_some() || self.scanned.is_some_and(|at| now < at + RESCAN) {
            return;
        }
        self.scanned = Some(now);
        let ids = gamepads.gamepads().unwrap_or_default();
        self.any_controller = !ids.is_empty();
        for id in ids {
            let Ok(pad) = gamepads.open(id) else { continue };
            // SAFETY: the gamepad is open, as SDL_GamepadHasSensor needs
            if unsafe { pad.has_sensor(SensorType::Gyroscope) } && pad.sensor_set_enabled(SensorType::Gyroscope, true).is_ok() {
                let name = pad.name().unwrap_or_else(|| "a controller".to_owned());
                log::info!("aiming with the gyroscope of {name}");
                self.pad = Some((pad, name));
                return;
            }
        }
    }

    /// reads the controller, every pass of the host's loop whether the game
    /// runs or not, so a calibration started in the settings takes its
    /// second of still readings while they are open.
    pub fn update(&mut self) {
        let now = Instant::now();
        self.scan(now);
        self.reading = [0.0; 3];
        let Some((pad, _)) = &self.pad else { return };
        let mut reading = [0.0f32; 3];
        if pad.sensor_get_data(SensorType::Gyroscope, &mut reading).is_err() {
            return;
        }
        // SDL gives radians a second, x pitch, y yaw and z roll as the
        // controller is held, the console's own axes held as it is
        let reading = reading.map(f32::to_degrees);
        if let Some((started, sum, count)) = &mut self.calibrating {
            for (sum, value) in sum.iter_mut().zip(reading) {
                *sum += value;
            }
            *count += 1;
            if now >= *started + CALIBRATION {
                let count = *count as f32;
                self.bias = sum.map(|sum| sum / count);
                self.calibrating = None;
                log::info!("gyroscope calibrated, rest {:?}", self.bias);
            }
            return;
        }
        self.reading = [0, 1, 2].map(|axis| reading[axis] - self.bias[axis]);
    }

    /// how fast the controller turned at the last update, in degrees a
    /// second about the console's x, y and z axes, as the aim preferences
    /// want it: nothing with the gyro off, not held for Hold, outside a
    /// battle or while calibrating.
    pub fn turning(&self, aim: &Aim, battle: bool, held: bool) -> [f32; 3] {
        let on = match aim.gyro {
            GyroMode::Off => false,
            GyroMode::Always => true,
            GyroMode::Hold => held,
        };
        if !on || !battle {
            return [0.0; 3];
        }
        let flip = |inverted: bool| if inverted { -1.0 } else { 1.0 };
        let [x, y, z] = self.reading.map(|rate| rate * aim.gyro_sensitivity);
        [x * flip(aim.gyro_invert_y), y * flip(aim.gyro_invert_x), z]
    }
}
