//! what the user chose, kept between runs in the system's place for settings

use std::path::{Path, PathBuf};

use gilrs::Button;
use serde::{Deserialize, Serialize};
use winit::keyboard::KeyCode;
use zakuro_core::services::hid::PadState;
use zakuro_gpu::{ScreenLayout, ScreenOrientation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Renderer {
    #[default]
    Vulkan,
    OpenGl,
}

/// the most pictures a second High Refresh Mode shows, or Off to follow the
/// display's refresh rate. it never goes below 60, so the mode can't show
/// fewer pictures than the 60 FPS patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HighRefreshCap {
    #[default]
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "90")]
    Hz90,
    #[serde(rename = "120")]
    Hz120,
    #[serde(rename = "144")]
    Hz144,
    #[serde(rename = "165")]
    Hz165,
    #[serde(rename = "240")]
    Hz240,
}

impl HighRefreshCap {
    #[cfg(test)]
    pub const ALL: [HighRefreshCap; 6] =
        [HighRefreshCap::Off, HighRefreshCap::Hz90, HighRefreshCap::Hz120, HighRefreshCap::Hz144, HighRefreshCap::Hz165, HighRefreshCap::Hz240];

    pub fn name(self) -> &'static str {
        match self {
            HighRefreshCap::Off => "Off, match the display",
            HighRefreshCap::Hz90 => "90 Hz",
            HighRefreshCap::Hz120 => "120 Hz",
            HighRefreshCap::Hz144 => "144 Hz",
            HighRefreshCap::Hz165 => "165 Hz",
            HighRefreshCap::Hz240 => "240 Hz",
        }
    }

    /// the pictures a second it caps the mode at, none when it follows the
    /// display.
    pub fn hz(self) -> Option<u32> {
        match self {
            HighRefreshCap::Off => None,
            HighRefreshCap::Hz90 => Some(90),
            HighRefreshCap::Hz120 => Some(120),
            HighRefreshCap::Hz144 => Some(144),
            HighRefreshCap::Hz165 => Some(165),
            HighRefreshCap::Hz240 => Some(240),
        }
    }

    /// the longest gap between two pictures, the cap's or the display's,
    /// whichever is slower: a cap never raises the rate past the display's.
    pub fn step(self, display: std::time::Duration) -> std::time::Duration {
        match self.hz() {
            Some(hz) => display.max(std::time::Duration::from_secs_f64(1.0 / f64::from(hz))),
            None => display,
        }
    }
}

/// how the screens are arranged in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Screens {
    #[default]
    Stacked,
    SideBySide,
    TopOnly,
    BottomOnly,
    LargeTop,
}

impl Screens {
    pub const ALL: [Screens; 5] = [Screens::Stacked, Screens::SideBySide, Screens::LargeTop, Screens::TopOnly, Screens::BottomOnly];

    pub fn name(self) -> &'static str {
        match self {
            Screens::Stacked => "Top over bottom",
            Screens::SideBySide => "Screens side by side",
            Screens::TopOnly => "Top screen only",
            Screens::BottomOnly => "Bottom screen only",
            Screens::LargeTop => "Big top screen, small bottom one",
        }
    }

    pub fn screens(self) -> ScreenLayout {
        match self {
            Screens::Stacked => ScreenLayout::Stacked,
            Screens::SideBySide => ScreenLayout::SideBySide,
            Screens::TopOnly => ScreenLayout::TopOnly,
            Screens::BottomOnly => ScreenLayout::BottomOnly,
            Screens::LargeTop => ScreenLayout::LargeTop,
        }
    }

    /// the one after it, going round
    pub fn next(self) -> Screens {
        let at = Screens::ALL.iter().position(|&layout| layout == self).unwrap_or(0);
        Screens::ALL[(at + 1) % Screens::ALL.len()]
    }

    /// w/ one screen showing, the other one alone, otherwise the same
    pub fn other_screen(self) -> Screens {
        match self {
            Screens::TopOnly => Screens::BottomOnly,
            Screens::BottomOnly => Screens::TopOnly,
            both => both,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    #[default]
    North,
    East,
    South,
    West,
}

impl Orientation {
    pub const ALL: [Orientation; 4] = [Orientation::North, Orientation::East, Orientation::South, Orientation::West];

    pub fn name(self) -> &'static str {
        match self {
            Orientation::North => "0 degree",
            Orientation::East => "90 degree",
            Orientation::South => "180 degree",
            Orientation::West => "270 degree",
        }
    }

    pub fn screen_orientation(self) -> ScreenOrientation {
        match self {
            Orientation::North => ScreenOrientation::North,
            Orientation::East => ScreenOrientation::East,
            Orientation::South => ScreenOrientation::South,
            Orientation::West => ScreenOrientation::West,
        }
    }
}

/// how the screens' pixels look scaled up to the window
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    #[default]
    Smooth,
    Sharp,
    Pixels,
}

impl Filter {
    pub const ALL: [Filter; 3] = [Filter::Smooth, Filter::Sharp, Filter::Pixels];

    pub fn name(self) -> &'static str {
        match self {
            Filter::Smooth => "Smooth",
            Filter::Sharp => "Sharp",
            Filter::Pixels => "Pixels",
        }
    }

    pub fn screens(self) -> zakuro_gpu::ScreenFilter {
        match self {
            Filter::Smooth => zakuro_gpu::ScreenFilter::Smooth,
            Filter::Sharp => zakuro_gpu::ScreenFilter::Sharp,
            Filter::Pixels => zakuro_gpu::ScreenFilter::Pixels,
        }
    }
}

/// the top screen's shape
/// a game draws into 400x240 whatever this is,
/// so anything but the console's own only looks right w/ a widescreen code on,
/// which makes the game draw a wider scene into those pixels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopShape {
    #[default]
    Console,
    Wide16x9,
    Wide21x9,
    /// the window's shape, with the top screen alone, see App::top_width.
    Fit,
}

impl TopShape {
    pub const ALL: [TopShape; 4] = [TopShape::Console, TopShape::Wide16x9, TopShape::Wide21x9, TopShape::Fit];

    pub fn name(self) -> &'static str {
        match self {
            TopShape::Console => "The console's, 5:3",
            TopShape::Wide16x9 => "16:9",
            TopShape::Wide21x9 => "21:9",
            TopShape::Fit => "The window's, with the top screen alone",
        }
    }

    /// the top screen's width at its height of 240, the console's for one
    /// that fits the window when the window is sized to the screens.
    pub fn width(self) -> f32 {
        match self {
            TopShape::Console | TopShape::Fit => 400.0,
            TopShape::Wide16x9 => 240.0 * 16.0 / 9.0,
            TopShape::Wide21x9 => 560.0,
        }
    }
}

/// how the top screen's 3D shows, for games that draw it
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stereo3d {
    /// games draw flat, like w/ the console's 3D slider down
    #[default]
    Off,
    Anaglyph,
    SideBySide,
    HalfSideBySide,
    Interlaced,
}

impl Stereo3d {
    pub fn mode(self) -> zakuro_gpu::StereoMode {
        match self {
            Stereo3d::Off => zakuro_gpu::StereoMode::Off,
            Stereo3d::Anaglyph => zakuro_gpu::StereoMode::Anaglyph,
            Stereo3d::SideBySide => zakuro_gpu::StereoMode::SideBySide,
            Stereo3d::HalfSideBySide => zakuro_gpu::StereoMode::HalfSideBySide,
            Stereo3d::Interlaced => zakuro_gpu::StereoMode::Interlaced,
        }
    }
}

/// the console's system language, a title reads it from cfg:u and picks its localized text and files by it,
/// the codes the console numbers them by
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Japanese,
    #[default]
    English,
    French,
    German,
    Italian,
    Spanish,
    ChineseSimplified,
    Korean,
    Dutch,
    Portuguese,
    Russian,
    ChineseTraditional,
}

impl Language {
    pub const ALL: [Language; 12] = [
        Language::Japanese,
        Language::English,
        Language::French,
        Language::German,
        Language::Italian,
        Language::Spanish,
        Language::ChineseSimplified,
        Language::Korean,
        Language::Dutch,
        Language::Portuguese,
        Language::Russian,
        Language::ChineseTraditional,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Language::Japanese => "Japanese",
            Language::English => "English",
            Language::French => "French",
            Language::German => "German",
            Language::Italian => "Italian",
            Language::Spanish => "Spanish",
            Language::ChineseSimplified => "Chinese (Simplified)",
            Language::Korean => "Korean",
            Language::Dutch => "Dutch",
            Language::Portuguese => "Portuguese",
            Language::Russian => "Russian",
            Language::ChineseTraditional => "Chinese (Traditional)",
        }
    }

    /// the number the console's blocks give the language
    pub fn code(self) -> u8 {
        match self {
            Language::Japanese => 0,
            Language::English => 1,
            Language::French => 2,
            Language::German => 3,
            Language::Italian => 4,
            Language::Spanish => 5,
            Language::ChineseSimplified => 6,
            Language::Korean => 7,
            Language::Dutch => 8,
            Language::Portuguese => 9,
            Language::Russian => 10,
            Language::ChineseTraditional => 11,
        }
    }
}

/// what the console's cameras see
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CameraSource {
    /// the computer's camera, on only while a game takes pictures
    #[default]
    Webcam,
    /// a picture from a file, like a QR code's
    Picture,
    /// nothing, black
    Nothing,
}

impl CameraSource {
    pub const ALL: [CameraSource; 3] = [CameraSource::Webcam, CameraSource::Picture, CameraSource::Nothing];

    pub fn name(self) -> &'static str {
        match self {
            CameraSource::Webcam => "The computer's camera",
            CameraSource::Picture => "A picture",
            CameraSource::Nothing => "Nothing",
        }
    }
}

/// what the console's microphone hears
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MicrophoneSource {
    /// the computer's mic, only open while a game listens
    Computer,
    /// nothing except the blowing key
    /// the default, since some games listen all the time (Super Mario 3D Land from its start) and the computer's
    /// mic would stay open w/ them
    #[default]
    Nothing,
}

impl MicrophoneSource {
    pub const ALL: [MicrophoneSource; 2] = [MicrophoneSource::Computer, MicrophoneSource::Nothing];

    pub fn name(self) -> &'static str {
        match self {
            MicrophoneSource::Computer => "The computer's microphone",
            MicrophoneSource::Nothing => "Nothing",
        }
    }
}

/// the keys the console's buttons, circle pad and C stick are on
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Keys {
    pub a: KeyCode,
    pub b: KeyCode,
    pub x: KeyCode,
    pub y: KeyCode,
    pub l: KeyCode,
    pub r: KeyCode,
    pub start: KeyCode,
    pub select: KeyCode,
    pub up: KeyCode,
    pub down: KeyCode,
    pub left: KeyCode,
    pub right: KeyCode,
    pub circle_up: KeyCode,
    pub circle_down: KeyCode,
    pub circle_left: KeyCode,
    pub circle_right: KeyCode,
    pub zl: KeyCode,
    pub zr: KeyCode,
    pub c_up: KeyCode,
    pub c_down: KeyCode,
    pub c_left: KeyCode,
    pub c_right: KeyCode,
    /// uses the current power in a battle, as touching its box does.
    pub power: KeyCode,
    pub vehicle_ability: KeyCode,
    pub vehicle_exit: KeyCode,
}

impl Default for Keys {
    fn default() -> Self {
        Keys {
            a: KeyCode::KeyX,
            b: KeyCode::KeyZ,
            x: KeyCode::KeyS,
            y: KeyCode::KeyA,
            l: KeyCode::KeyQ,
            r: KeyCode::KeyW,
            start: KeyCode::Enter,
            select: KeyCode::Backspace,
            up: KeyCode::ArrowUp,
            down: KeyCode::ArrowDown,
            left: KeyCode::ArrowLeft,
            right: KeyCode::ArrowRight,
            circle_up: KeyCode::KeyI,
            circle_down: KeyCode::KeyK,
            circle_left: KeyCode::KeyJ,
            circle_right: KeyCode::KeyL,
            zl: KeyCode::Digit3,
            zr: KeyCode::Digit2,
            c_up: KeyCode::KeyT,
            c_down: KeyCode::KeyG,
            c_left: KeyCode::KeyF,
            c_right: KeyCode::KeyH,
            power: KeyCode::Digit1,
            vehicle_ability: KeyCode::Digit4,
            vehicle_exit: KeyCode::Digit5,
        }
    }
}

impl Keys {
    /// is a console button on key
    pub fn binds(&self, key: KeyCode) -> bool {
        self.clone().all_mut().into_iter().any(|(_, bound)| *bound == key)
    }

    /// every binding with the name it goes by, to show and change them.
    pub fn all_mut(&mut self) -> [(&'static str, &mut KeyCode); 25] {
        [
            ("A", &mut self.a),
            ("B", &mut self.b),
            ("X", &mut self.x),
            ("Y", &mut self.y),
            ("L", &mut self.l),
            ("R", &mut self.r),
            ("Start", &mut self.start),
            ("Select", &mut self.select),
            ("D-pad up", &mut self.up),
            ("D-pad down", &mut self.down),
            ("D-pad left", &mut self.left),
            ("D-pad right", &mut self.right),
            ("Circle pad up", &mut self.circle_up),
            ("Circle pad down", &mut self.circle_down),
            ("Circle pad left", &mut self.circle_left),
            ("Circle pad right", &mut self.circle_right),
            ("ZL", &mut self.zl),
            ("ZR", &mut self.zr),
            ("C-stick up", &mut self.c_up),
            ("C-stick down", &mut self.c_down),
            ("C-stick left", &mut self.c_left),
            ("C-stick right", &mut self.c_right),
            ("Use power", &mut self.power),
            ("Vehicle ability", &mut self.vehicle_ability),
            ("Exit vehicle", &mut self.vehicle_exit),
        ]
    }
}

/// the controller buttons the console's buttons are on, the face buttons where the 3DS has them instead of by their letters
/// the circle pad is the left stick and the C stick the right one
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PadButtons {
    pub a: Button,
    pub b: Button,
    pub x: Button,
    pub y: Button,
    pub l: Button,
    pub r: Button,
    pub start: Button,
    pub select: Button,
    pub up: Button,
    pub down: Button,
    pub left: Button,
    pub right: Button,
    pub zl: Button,
    pub zr: Button,
    pub vehicle_ability: Button,
    pub vehicle_exit: Button,
}

impl Default for PadButtons {
    fn default() -> Self {
        PadButtons {
            a: Button::East,
            b: Button::South,
            x: Button::North,
            y: Button::West,
            l: Button::LeftTrigger,
            r: Button::RightTrigger,
            start: Button::Start,
            select: Button::Select,
            up: Button::DPadUp,
            down: Button::DPadDown,
            left: Button::DPadLeft,
            right: Button::DPadRight,
            zl: Button::LeftTrigger2,
            zr: Button::RightTrigger2,
            vehicle_ability: Button::LeftThumb,
            vehicle_exit: Button::RightThumb,
        }
    }
}

impl PadButtons {
    /// every binding with the name it goes by, to show and change them.
    pub fn all_mut(&mut self) -> [(&'static str, &mut Button); 16] {
        [
            ("A", &mut self.a),
            ("B", &mut self.b),
            ("X", &mut self.x),
            ("Y", &mut self.y),
            ("L", &mut self.l),
            ("R", &mut self.r),
            ("Start", &mut self.start),
            ("Select", &mut self.select),
            ("D-pad up", &mut self.up),
            ("D-pad down", &mut self.down),
            ("D-pad left", &mut self.left),
            ("D-pad right", &mut self.right),
            ("ZL", &mut self.zl),
            ("ZR", &mut self.zr),
            ("Vehicle ability", &mut self.vehicle_ability),
            ("Exit vehicle", &mut self.vehicle_exit),
        ]
    }

    /// each of the console's buttons and the controller button it's on
    pub fn map(&self) -> [(PadState, Button); 14] {
        [
            (PadState::A, self.a),
            (PadState::B, self.b),
            (PadState::X, self.x),
            (PadState::Y, self.y),
            (PadState::L, self.l),
            (PadState::R, self.r),
            (PadState::START, self.start),
            (PadState::SELECT, self.select),
            (PadState::UP, self.up),
            (PadState::DOWN, self.down),
            (PadState::LEFT, self.left),
            (PadState::RIGHT, self.right),
            (PadState::ZL, self.zl),
            (PadState::ZR, self.zr),
        ]
    }
}

/// what a game has of its own over the general settings, each one left out follows them
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameSettings {
    pub hardware_rasterizer: Option<bool>,
    pub resolution: Option<u32>,
    pub texture_packs: Option<bool>,
    pub frame_interpolation: Option<bool>,
    pub scale: Option<u32>,
    #[serde(deserialize_with = "or_default")]
    pub layout: Option<Screens>,
    #[serde(deserialize_with = "or_default")]
    pub orientation: Option<Orientation>,
    #[serde(deserialize_with = "or_default")]
    pub filter: Option<Filter>,
    pub integer_scale: Option<bool>,
    #[serde(deserialize_with = "or_default")]
    pub top_shape: Option<TopShape>,
    #[serde(deserialize_with = "or_default")]
    pub stereo: Option<Stereo3d>,
    pub depth_3d: Option<u32>,
    #[serde(deserialize_with = "or_default")]
    pub language: Option<Language>,
    pub recompiled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// the folder the library lists games from
    pub games: Option<PathBuf>,
    /// the console's system language, titles pick their localized text and files by it
    #[serde(deserialize_with = "or_default")]
    pub language: Language,
    #[serde(deserialize_with = "or_default")]
    pub renderer: Renderer,
    /// draw the 3D on the GPU instead of in software
    pub hardware_rasterizer: bool,
    /// how many times the console's res the GPU draws the 3D at
    pub resolution: u32,
    /// draw the texture packs put in games' mods folders
    pub texture_packs: bool,
    /// a new picture on every refresh of the display, drawn in between the
    /// last two the game made. hidden and always off for now: it slows the
    /// emulation down, so a saved value is neither read nor written.
    #[serde(skip)]
    pub high_refresh_mode: bool,
    /// the most pictures a second High Refresh Mode shows.
    #[serde(deserialize_with = "or_default")]
    pub high_refresh_cap: HighRefreshCap,
    /// run a game's recompiled code when it has some, rather than
    /// interpreting everything.
    pub recompiled: bool,
    /// window size, times the console's
    pub scale: u32,
    #[serde(deserialize_with = "or_default")]
    pub layout: Screens,
    #[serde(deserialize_with = "or_default")]
    pub orientation: Orientation,
    /// how the screens' pixels look scaled up
    #[serde(deserialize_with = "or_default")]
    pub filter: Filter,
    /// only scale the screens by whole numbers, leaving a border
    pub integer_scale: bool,
    /// how wide the top screen shows
    #[serde(deserialize_with = "or_default")]
    pub top_shape: TopShape,
    /// how the 3D shows. hidden and always off: Kid Icarus's right eye is
    /// no longer drawn, so a saved value is neither read nor written.
    #[serde(skip)]
    pub stereo: Stereo3d,
    /// how deep games draw the 3D, the console's slider, 0 to 100.
    #[serde(skip)]
    pub depth_3d: u32,
    /// each eye gets the other's picture.
    #[serde(skip)]
    pub swap_eyes: bool,
    /// 0 to 1
    pub volume: f32,
    pub mute: bool,
    pub show_fps: bool,
    /// a picture behind the library
    pub background: Option<PathBuf>,
    /// how much of it shows, 0 to 1
    pub background_opacity: f32,
    pub keys: Keys,
    pub pad: PadButtons,
    /// the enhancements turned on, by name, for each game by its title id
    pub enhancements: std::collections::BTreeMap<String, Vec<String>>,
    /// what each game has of its own, by its title id
    pub per_game: std::collections::BTreeMap<String, GameSettings>,
    /// what the console's cameras see, and the picture when it's one
    #[serde(deserialize_with = "or_default")]
    pub camera: CameraSource,
    pub camera_picture: Option<PathBuf>,
    /// what the console's mic hears
    #[serde(deserialize_with = "or_default")]
    pub microphone: MicrophoneSource,
    /// the name the player goes by in rooms, the console's own too
    pub nickname: String,
    /// the number that tells this console apart over local wireless, made once
    pub console_id: u64,
    /// the room last joined, and the one opened, its name and port
    pub room_address: String,
    pub room_name: String,
    pub room_port: u16,
    /// Kid Icarus: Uprising's PC Preferences, shown in its own Options.
    pub pc: crate::preferences::PcPreferences,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            games: None,
            language: Language::English,
            renderer: Renderer::Vulkan,
            hardware_rasterizer: true,
            resolution: 3,
            texture_packs: true,
            high_refresh_mode: false,
            high_refresh_cap: HighRefreshCap::Off,
            recompiled: true,
            scale: 2,
            layout: Screens::Stacked,
            orientation: Orientation::North,
            filter: Filter::Smooth,
            integer_scale: false,
            top_shape: TopShape::Console,
            stereo: Stereo3d::Off,
            depth_3d: 50,
            swap_eyes: false,
            volume: 1.0,
            mute: false,
            show_fps: false,
            background: None,
            background_opacity: 0.35,
            keys: Keys::default(),
            pad: PadButtons::default(),
            enhancements: std::collections::BTreeMap::new(),
            per_game: std::collections::BTreeMap::new(),
            camera: CameraSource::Webcam,
            camera_picture: None,
            microphone: MicrophoneSource::Nothing,
            nickname: String::new(),
            console_id: 0,
            room_address: String::new(),
            room_name: String::new(),
            room_port: zakuro_core::room::DEFAULT_PORT,
            pc: crate::preferences::PcPreferences::default(),
        }
    }
}

/// a setting's value, or its default when the file holds one this build doesn't know (a newer Zakuro's say),
/// so one value doesn't lose all the others
fn or_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    let value = toml::Value::deserialize(deserializer)?;
    Ok(T::deserialize(value).unwrap_or_default())
}

impl GameSettings {
    /// the settings w/ these over them
    fn over(&self, settings: &mut Settings) {
        settings.hardware_rasterizer = self.hardware_rasterizer.unwrap_or(settings.hardware_rasterizer);
        settings.resolution = self.resolution.unwrap_or(settings.resolution);
        settings.texture_packs = self.texture_packs.unwrap_or(settings.texture_packs);
        // frame_interpolation is kept for files that have it, but High Refresh Mode, which took its place, is never saved
        settings.scale = self.scale.unwrap_or(settings.scale);
        settings.layout = self.layout.unwrap_or(settings.layout);
        settings.orientation = self.orientation.unwrap_or(settings.orientation);
        settings.filter = self.filter.unwrap_or(settings.filter);
        settings.integer_scale = self.integer_scale.unwrap_or(settings.integer_scale);
        settings.top_shape = self.top_shape.unwrap_or(settings.top_shape);
        settings.stereo = self.stereo.unwrap_or(settings.stereo);
        settings.depth_3d = self.depth_3d.unwrap_or(settings.depth_3d);
        settings.language = self.language.unwrap_or(settings.language);
        settings.recompiled = self.recompiled.unwrap_or(settings.recompiled);
    }
}

/// what a game goes by in the settings file, its title id
pub fn game_key(program_id: u64) -> String {
    format!("{program_id:016X}")
}

impl Settings {
    /// the settings a game plays w/, or the general ones w/o a game
    pub fn for_game(&self, program_id: Option<u64>) -> Settings {
        let mut settings = self.clone();
        if let Some(own) = program_id.and_then(|id| self.per_game.get(&game_key(id))) {
            own.over(&mut settings);
        }
        settings
    }

    /// does a game have anything of its own
    pub fn has_own(&self, program_id: u64) -> bool {
        self.per_game.contains_key(&game_key(program_id))
    }

    /// where the console's 3D slider is, down while the 3D is off, so games don't draw a picture nobody sees
    pub fn slider_3d(&self) -> f32 {
        if self.stereo == Stereo3d::Off {
            0.0
        } else {
            self.depth_3d.min(100) as f32 / 100.0
        }
    }

    /// the 3D the presenter shows
    pub fn stereo(&self) -> zakuro_gpu::Stereo {
        zakuro_gpu::Stereo { mode: self.stereo.mode(), swap: self.swap_eyes }
    }

    /// the saved settings, or the defaults when there are none
    pub fn load() -> Settings {
        let Some(path) = path() else { return Settings::default() };
        match std::fs::read_to_string(&path) {
            Ok(text) => Settings::parse(&text).unwrap_or_else(|error| {
                // kept aside, the next save would write over it
                let aside = path.with_extension("toml.bad");
                let _ = std::fs::copy(&path, &aside);
                log::warn!("could not read {}, {error}, using the defaults, it is kept as {}", path.display(), aside.display());
                Settings::default()
            }),
            Err(_) => Settings::default(),
        }
    }

    /// settings from a file's text. a file from before the PC Preferences
    /// gets its changed console keys as Action Bindings where they have one
    /// meaning, and the rest listed for the player to review.
    pub fn parse(text: &str) -> Result<Settings, toml::de::Error> {
        let mut settings: Settings = toml::from_str(text)?;
        let older = text.parse::<toml::Table>().is_ok_and(|table| !table.contains_key("pc"));
        if older {
            settings.pc.bindings = crate::bindings::migrate(&settings.keys, &settings.pad);
            settings.pc.review = crate::bindings::unmigrated(&settings.keys, &settings.pad).into_iter().map(str::to_owned).collect();
        }
        Ok(settings)
    }

    /// writes the settings, through a file beside them renamed over them,
    /// so a failed write leaves the last ones whole.
    pub fn save(&self) -> Result<(), String> {
        let path = path().ok_or("there is no place for settings on this system")?;
        self.save_to(&path).map_err(|error| {
            log::warn!("could not save the settings to {}, {error}", path.display());
            error
        })
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string_pretty(self).map_err(|error| error.to_string())?;
        write(path, &text).map_err(|error| format!("{} ({error})", path.display()))
    }
}

fn write(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let fresh = path.with_extension("toml.new");
    std::fs::write(&fresh, text)?;
    std::fs::rename(&fresh, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&fresh);
    })
}

/// the system's place for a program's settings
fn path() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    let dir = if cfg!(windows) {
        var("APPDATA")
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|home| home.join("Library/Application Support"))
    } else {
        var("XDG_CONFIG_HOME").or_else(|| var("HOME").map(|home| home.join(".config")))
    };
    dir.map(|dir| dir.join("zakuro").join("settings.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_come_back_as_they_were_saved() {
        let mut settings =
            Settings { games: Some("/games".into()), show_fps: true, layout: Screens::TopOnly, language: Language::German, ..Settings::default() };
        settings.keys.a = KeyCode::KeyK;
        settings.pad.a = Button::South;
        let text = toml::to_string_pretty(&settings).unwrap();
        assert_eq!(toml::from_str::<Settings>(&text).unwrap(), settings);
    }

    #[test]
    fn a_value_this_build_does_not_know_keeps_the_others() {
        let settings: Settings = toml::from_str("show_fps = true\nlayout = \"three_screens\"\nfilter = \"crt\"\n").unwrap();
        assert!(settings.show_fps);
        assert_eq!(settings.layout, Screens::default());
        assert_eq!(settings.filter, Filter::default());
    }

    #[test]
    fn missing_fields_take_their_defaults() {
        let settings: Settings = toml::from_str("show_fps = true\n[keys]\na = \"KeyK\"\n").unwrap();
        assert!(settings.show_fps);
        assert_eq!(settings.keys.a, KeyCode::KeyK);
        assert_eq!(settings.keys.b, Keys::default().b);
        assert_eq!(settings.scale, 2);
        assert_eq!(settings.layout, Screens::Stacked);
    }

    /// a game plays w/ what it has of its own and w/ the general settings in everything else, and no other game gets touched
    #[test]
    fn a_game_has_its_own_settings_over_the_general_ones() {
        let mut settings = Settings { resolution: 2, layout: Screens::SideBySide, ..Settings::default() };
        settings.per_game.insert(
            game_key(0x0004_0000_0012_3400),
            GameSettings { orientation: Some(Orientation::East), resolution: Some(5), recompiled: Some(false), ..GameSettings::default() },
        );
        let game = settings.for_game(Some(0x0004_0000_0012_3400));
        assert_eq!(game.orientation, Orientation::East);
        assert_eq!(game.resolution, 5);
        assert!(!game.recompiled);
        assert_eq!(game.layout, Screens::SideBySide);
        for other in [Some(0x0004_0000_0012_3500), None] {
            let general = settings.for_game(other);
            assert_eq!(general, settings);
        }
        assert!(settings.has_own(0x0004_0000_0012_3400));
        assert!(!settings.has_own(0x0004_0000_0012_3500));
    }

    #[test]
    fn a_games_settings_come_back_as_they_were_saved() {
        let mut settings = Settings::default();
        settings.per_game.insert(
            game_key(0x0004_0000_0012_3400),
            GameSettings { orientation: Some(Orientation::West), language: Some(Language::Japanese), texture_packs: Some(false), ..GameSettings::default() },
        );
        let text = toml::to_string_pretty(&settings).unwrap();
        let own = text.split("[per_game.").nth(1).unwrap();
        assert!(!own.contains("resolution"), "only what it has of its own is written for a game");
        assert_eq!(toml::from_str::<Settings>(&text).unwrap(), settings);
    }

    /// a value from a newer Zakuro leaves the game following the general setting, and takes nothing else w/ it
    #[test]
    fn a_value_this_build_does_not_know_follows_the_general_setting() {
        let text = "[per_game.0004000000123400]\nlayout = \"three_screens\"\nresolution = 4\n";
        let settings: Settings = toml::from_str(text).unwrap();
        let own = &settings.per_game[&game_key(0x0004_0000_0012_3400)];
        assert_eq!(own.layout, None);
        assert_eq!(own.resolution, Some(4));
    }

    #[test]
    fn layouts_go_round() {
        let mut layout = Screens::Stacked;
        for _ in 0..Screens::ALL.len() {
            layout = layout.next();
        }
        assert_eq!(layout, Screens::Stacked);
    }

    /// High Refresh Mode stays off whatever a settings file says, under
    /// its name or the old frame interpolation one, and is not written.
    #[test]
    fn saved_high_refresh_mode_stays_off() {
        for text in ["high_refresh_mode = true\n", "frame_interpolation = true\n"] {
            let settings: Settings = toml::from_str(text).unwrap();
            assert!(!settings.high_refresh_mode, "{text}");
        }
        let text = toml::to_string_pretty(&Settings::default()).unwrap();
        assert!(!text.contains("high_refresh_mode") && !text.contains("frame_interpolation"), "{text}");
    }

    /// the 3D stays off whatever a settings file says, so the console's
    /// slider stays down, and it is not written.
    #[test]
    fn saved_3d_stays_off() {
        let settings: Settings = toml::from_str("stereo = \"anaglyph\"\ndepth_3d = 80\nswap_eyes = true\n").unwrap();
        assert_eq!(settings.stereo, Stereo3d::Off);
        assert_eq!(settings.slider_3d(), 0.0);
        let text = toml::to_string_pretty(&Settings::default()).unwrap();
        assert!(!text.contains("stereo") && !text.contains("depth_3d") && !text.contains("swap_eyes"), "{text}");
    }

    /// the cap offers only rates at or above 60, and at 240 Hz it brings
    /// the pictures down to 120 a second.
    #[test]
    fn a_120_cap_at_240_hz_shows_120_pictures_a_second() {
        for cap in HighRefreshCap::ALL {
            if let Some(hz) = cap.hz() {
                assert!(hz >= 60, "{cap:?} is below 60");
            }
        }
        assert_eq!(HighRefreshCap::ALL.len(), 6);
        let display = std::time::Duration::from_secs_f64(1.0 / 240.0);
        let step = HighRefreshCap::Hz120.step(display);
        // a nanosecond's rounding of the interval, so not exactly 120.0
        let rate = 1.0 / step.as_secs_f64();
        assert!((rate - 120.0).abs() < 0.001, "{rate} pictures a second");
        // off follows the display, and a cap faster than it does not raise
        // the rate past it
        assert_eq!(HighRefreshCap::Off.step(display), display);
        let sixty = std::time::Duration::from_secs_f64(1.0 / 60.0);
        assert_eq!(HighRefreshCap::Hz240.step(sixty), sixty);
    }

    /// the cap is kept between runs, and a value this build does not know,
    /// below 60 say, reads as Off without losing the other settings.
    #[test]
    fn the_high_refresh_cap_persists_and_reads_back() {
        assert_eq!(HighRefreshCap::default(), HighRefreshCap::Off);
        let settings = Settings { high_refresh_cap: HighRefreshCap::Hz144, show_fps: true, ..Settings::default() };
        let text = toml::to_string_pretty(&settings).unwrap();
        assert!(text.contains("high_refresh_cap = \"144\""), "{text}");
        let back: Settings = toml::from_str(&text).unwrap();
        assert_eq!(back.high_refresh_cap, HighRefreshCap::Hz144);
        assert!(back.show_fps);

        let unknown: Settings = toml::from_str("high_refresh_cap = \"30\"\nshow_fps = true\n").unwrap();
        assert_eq!(unknown.high_refresh_cap, HighRefreshCap::Off);
        assert!(unknown.show_fps, "the other settings are kept");
    }

    /// a save that cannot be written says so and leaves the last settings
    /// as they were.
    #[test]
    fn a_failed_save_keeps_the_last_settings() {
        let dir = std::env::temp_dir().join(format!("zakuro-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.toml");
        let saved = Settings { volume: 0.25, ..Settings::default() };
        saved.save_to(&path).unwrap();
        // the file beside it a directory, so the new one can't be written
        std::fs::create_dir_all(path.with_extension("toml.new")).unwrap();
        let error = Settings { volume: 0.75, ..Settings::default() }.save_to(&path);
        assert!(error.is_err());
        let back: Settings = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back.volume, 0.25);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
