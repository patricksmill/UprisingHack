//! Zakuro's frontend, a window showing the library of games or a game with
//! a menu over it, and the loop that drives the emulated console one frame
//! at a time.

mod audio;
mod bindings;
mod camera;
mod cli;
mod gamepad;
mod gui;
mod gyro;
mod input;
mod library;
mod menus;
mod preferences;
mod present;
mod recompile;
mod report;
mod settings;
mod zig;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId};

use zakuro_common::Screen;
use zakuro_core::aim_mouse::{CauldronRules, DialogRules, NavigationLink, PauseRules};
use zakuro_core::{loader, Config, FrameOutcome, System};
use zakuro_gpu::{layout, GpuScreen, Overlay, PresentError, RendererKind, ScreenImage};

use gui::Gui;
use input::Keyboard;
use library::Library;
use menus::{Action, Menus};
use present::Backend;
use recompile::{Job, Stage, Toolchain};
use settings::{CameraSource, Renderer, Screens, Settings};

pub use zakuro_core::recompiled::Linked;

#[derive(serde::Serialize, serde::Deserialize)]
struct SavedLayout {
    /// Scripted Menu parts hidden by the editor, by screen and part, absent
    /// in older files. first, as TOML wants its values before its tables.
    #[serde(default)]
    hidden: Vec<(usize, usize)>,
    #[serde(default)]
    layers: Vec<SavedLayer>,
    /// absent in files written before Pause rules were kept.
    #[serde(default)]
    pause: Option<PauseRules>,
    /// the Retreat Dialog's rules, absent in older files.
    #[serde(default)]
    dialog: Option<DialogRules>,
    /// explicit Control-to-Control navigation links, absent in older files.
    #[serde(default)]
    navigation: Vec<NavigationLink>,
    /// the Fiend's Cauldron's rules, absent in older files. last, as its
    /// rules are an array of tables.
    #[serde(default)]
    cauldron: Option<CauldronRules>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SavedLayer {
    screen: usize,
    part: usize,
    to: [f32; 4],
}

fn layout_path(data_dir: &Option<PathBuf>) -> PathBuf {
    data_dir.clone().unwrap_or_default().join("menu-layout.toml")
}

fn load_layout(data_dir: &Option<PathBuf>) -> Result<SavedLayout, String> {
    let path = layout_path(data_dir);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SavedLayout { layers: Vec::new(), pause: None, dialog: None, navigation: Vec::new(), hidden: Vec::new(), cauldron: None })
        }
        Err(error) => return Err(format!("Could not read {}: {error}", path.display())),
    };
    toml::from_str(&text).map_err(|error| format!("Could not read {}: {error}", path.display()))
}

fn save_layout(
    data_dir: &Option<PathBuf>,
    layers: &[(usize, usize, [f32; 4])],
    pause: PauseRules,
    dialog: DialogRules,
    cauldron: CauldronRules,
    navigation: Vec<NavigationLink>,
    hidden: Vec<(usize, usize)>,
) -> Result<PathBuf, String> {
    let saved = SavedLayout {
        layers: layers.iter().map(|&(screen, part, to)| SavedLayer { screen, part, to }).collect(),
        pause: Some(pause),
        dialog: Some(dialog),
        navigation,
        cauldron: Some(cauldron),
        hidden,
    };
    let path = layout_path(data_dir);
    let text = toml::to_string_pretty(&saved).map_err(|error| error.to_string())?;
    std::fs::write(&path, text).map_err(|error| error.to_string())?;
    Ok(path)
}

/// runs the emulator as the command line says, on recompiled code linked
/// into the program when there is some.
pub fn run(linked: Option<Linked>) {
    report::init();

    let options = match cli::parse() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("zakuro: {message}");
            std::process::exit(2);
        }
    };
    let mut settings = Settings::load();
    // the console's own number, made the first time and kept, which tells
    // it apart from others over local wireless
    if settings.console_id == 0 {
        use std::hash::{BuildHasher, Hasher};
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |time| time.as_nanos()));
        settings.console_id = hasher.finish() | 1;
        let _ = settings.save();
    }

    let data_dir = options.data.clone().map(PathBuf::from).or_else(default_data_dir);
    let (layout_overrides, pause_rules, dialog_rules, cauldron_rules, navigation, hidden, layout_error) = if options.menu_editor {
        match load_layout(&data_dir) {
            Ok(saved) => (
                saved.layers.into_iter().map(|layer| (layer.screen, layer.part, layer.to)).collect(),
                saved.pause.unwrap_or_default(),
                saved.dialog.unwrap_or_default(),
                saved.cauldron.unwrap_or_default(),
                saved.navigation,
                saved.hidden,
                None,
            ),
            Err(error) => {
                log::warn!("{error}");
                (Vec::new(), PauseRules::default(), DialogRules::default(), CauldronRules::default(), Vec::new(), Vec::new(), Some(error))
            }
        }
    } else {
        (Vec::new(), PauseRules::default(), DialogRules::default(), CauldronRules::default(), Vec::new(), Vec::new(), None)
    };
    if let Some(dir) = &data_dir {
        report::to_file(dir);
        bring_saves(dir);
    }

    let menu_editor = options.menu_editor;
    let mut app = App {
        keyboard: Keyboard::new(settings.keys.clone()),
        gamepads: gamepad::Gamepads::new(settings.pad.clone()),
        gyro: gyro::Gyro::new(),
        prompt_faces: None,
        prompts_shown: None,
        scale: options.scale.unwrap_or(settings.scale).max(1),
        layout: settings.layout,
        top_shape: settings.top_shape,
        options,
        settings,
        linked,
        library: Library::new(data_dir.clone()),
        data_dir,
        layout_overrides,
        pause_rules,
        dialog_rules,
        cauldron_rules,
        navigation,
        hidden,
        game: None,
        menus: {
            let mut menus = Menus::default();
            menus.slot = 1;
            menus.editor_open = menu_editor;
            menus
        },
        jobs: Vec::new(),
        gui: None,
        backend: None,
        window: None,
        audio: None,
        mouse_down: false,
        modifiers: winit::keyboard::ModifiersState::empty(),
        tilting_from: None,
        aim_captured: false,
        released_for_pause: false,
        cursor: (0.0, 0.0),
        last_title_update: Instant::now(),
        next_frame: Instant::now(),
        skipped: 0,
        shown: FrameRate::new(),
        spent: Spent::default(),
        paused: false,
        fast_forward_key: false,
        fast_forwarding: false,
        next_shown: Instant::now(),
        display_step: Duration::from_nanos(1_000_000_000_000 / 60_000),
        host_step: Duration::from_nanos(1_000_000_000_000 / 60_000),
        next_host: Instant::now(),
        host_epoch: Instant::now(),
        frame_host: Duration::ZERO,
        older_frame_host: Duration::ZERO,
        shown_host: Duration::ZERO,
        high_refresh_applied: false,
        stop: false,
        camera: None,
        room_server: None,
        room: None,
        dlp_guest: None,
        dlp_own: None,
        wireless_reboot: None,
        maximized_before_full_screen: false,
        resize_after_full_screen: false,
    };

    if let Some(error) = layout_error {
        app.menus.message = Some(error);
    }

    if let Some(frames) = app.options.headless {
        app.start();
        if let Some(game) = &mut app.game {
            run_headless(&mut game.system, frames);
        }
        return;
    }

    if let Some(folder) = app.settings.games.clone() {
        app.library.scan(&folder);
    }
    app.audio = if app.options.mute {
        None
    } else {
        audio::Audio::open(zakuro_core::AUDIO_SAMPLE_RATE).inspect_err(|error| log::warn!("no sound, {error}")).ok()
    };
    app.apply_volume();

    let event_loop = EventLoop::new().expect("create an event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    if let Err(error) = event_loop.run_app(&mut app) {
        eprintln!("zakuro: {error}");
    }
}

/// opens a folder in the system's file manager.
fn show_folder(folder: &Path) {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    match std::process::Command::new(program).arg(folder).spawn() {
        // waited for so it leaves nothing behind when it closes
        Ok(mut child) => drop(std::thread::spawn(move || child.wait())),
        Err(error) => log::warn!("showing {}: {error}", folder.display()),
    }
}

/// the system's place for a program's data.
fn default_data_dir() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    let dir = if cfg!(windows) {
        var("APPDATA")
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|home| home.join("Library/Application Support"))
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|home| home.join(".local/share")))
    };
    dir.map(|dir| dir.join("zakuro"))
}

/// how long ago a state was saved, as a person would say it.
fn ago(time: std::time::SystemTime) -> String {
    let seconds = time.elapsed().map_or(0, |elapsed| elapsed.as_secs());
    match seconds {
        0..60 => "saved just now".into(),
        60..120 => "saved a minute ago".into(),
        120..3600 => format!("saved {} minutes ago", seconds / 60),
        3600..7200 => "saved an hour ago".into(),
        7200..86400 => format!("saved {} hours ago", seconds / 3600),
        86400..172800 => "saved yesterday".into(),
        _ => format!("saved {} days ago", seconds / 86400),
    }
}

/// saves Zakuro kept in the working directory before it had a place for
/// them, copied there the first time. the old ones stay as they were.
fn bring_saves(data: &Path) {
    let (old, new) = (Path::new("user"), data.join("user"));
    if new.exists() || !old.is_dir() {
        return;
    }
    // a copy cut short is left under another name, and tried again
    let partial = data.join("user.copying");
    let copied = std::fs::remove_dir_all(&partial)
        .or_else(|error| if error.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(error) })
        .and_then(|()| copy_dir(old, &partial))
        .and_then(|()| std::fs::rename(&partial, &new));
    match copied {
        Ok(()) => log::info!("copied the saves in ./user to {}, the old ones stay as they were", new.display()),
        Err(error) => log::warn!("could not copy the saves in ./user to {}, {error}", new.display()),
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// builds a System with no title loaded and both screens filled with a
/// solid color, to check the presentation path (framebuffer config -> guest
/// memory -> window) in isolation from any title's boot logic.
fn paint_test_pattern(config: Config) -> System {
    use zakuro_core::memory::{MemoryRegion, MemoryState, Permission};

    let mut system = System::new(config);

    // ABGR in memory for format 0 (Rgba8), red = 00 00 00 FF?
    let paint = |system: &mut System, screen: Screen, rgba: [u8; 4]| {
        let width = screen.width();
        let height = screen.height();
        let stride = height * 4;
        let size = stride * width;

        let block = system
            .memory
            .phys
            .allocate(MemoryRegion::Base, size)
            .expect("allocate test pattern framebuffer");

        let vaddr = zakuro_core::services::gsp::physical_to_virtual(system, block.addr);
        system.memory.map(
            vaddr,
            block.addr,
            size,
            Permission::READ | Permission::WRITE,
            MemoryState::Shared,
        );

        let mut pixel = [0u8; 4];
        pixel[3] = rgba[0]; // r
        pixel[2] = rgba[1]; // g
        pixel[1] = rgba[2]; // b
        pixel[0] = rgba[3]; // a
        let mut data = Vec::with_capacity(size as usize);
        for _ in 0..(size / 4) {
            data.extend_from_slice(&pixel);
        }
        system.memory.write_physical(block.addr, &data);

        let index = match screen {
            Screen::Top => 0,
            Screen::Bottom => 1,
        };
        system.set_framebuffer(index, 0, block.addr, block.addr, stride, 0);
    };

    paint(&mut system, Screen::Top, [220, 40, 40, 255]); // red
    paint(&mut system, Screen::Bottom, [40, 90, 220, 255]); // blue
    system
}

fn run_headless(system: &mut System, frames: u64) {
    let start = Instant::now();
    let mut outcome = FrameOutcome::Completed;
    let mut ran = 0;
    for _ in 0..frames {
        outcome = system.run_frame();
        ran += 1;
        if outcome != FrameOutcome::Completed {
            break;
        }
    }
    let elapsed = start.elapsed();
    println!("{outcome:?} after {ran} frames in {elapsed:.2?}");
    println!(
        "{:.1} MIPS, {}",
        system.cpu.cycles as f64 / elapsed.as_secs_f64() / 1e6,
        system.status_line()
    );
    if !system.fatal_errors.is_empty() {
        println!("fatal errors: {}", system.fatal_errors.join("; "));
    }
    for (thread, pc, hits) in system.hot_spots(10) {
        println!("  hot {thread:<8} 0x{pc:08X} {hits}");
    }
}

/// a game being played.
struct Running {
    system: System,
    path: PathBuf,
    name: String,
    /// the inputs written down, ZAKURO_RECORD=file, for playing the run
    /// back.
    recorder: Option<zakuro_core::replay::Recorder>,
    /// the inputs a recording wrote down, ZAKURO_REPLAY=file, played back in
    /// place of the keyboard's and the controllers', and the frames run.
    replay: Option<zakuro_core::replay::Replay>,
    frame: u64,
    /// the game's own frames a second, counted as they run.
    rate: FrameRate,
    /// the last rate the game's count read, kept for the menu.
    fps: f32,
}

impl Running {
    fn new(system: System, path: PathBuf, name: String) -> Running {
        Running {
            system,
            path,
            name,
            recorder: None,
            replay: None,
            frame: 0,
            rate: FrameRate::new(),
            fps: 0.0,
        }
    }

    fn count_frame(&mut self) {
        self.rate.count();
    }
}

struct App {
    options: cli::Options,
    settings: Settings,
    linked: Option<Linked>,
    data_dir: Option<PathBuf>,
    layout_overrides: Vec<(usize, usize, [f32; 4])>,
    /// the editor's Pause rules, kept across a game restart.
    pause_rules: PauseRules,
    /// the editor's Retreat Dialog rules, kept across a game restart.
    dialog_rules: DialogRules,
    /// the editor's Fiend's Cauldron rules, kept across a game restart.
    cauldron_rules: CauldronRules,
    /// the editor's explicit navigation links, kept across a restart and a
    /// console state load.
    navigation: Vec<NavigationLink>,
    /// the editor's hidden Scripted Menu parts, by screen and part, kept
    /// across a restart and a console state load.
    hidden: Vec<(usize, usize)>,
    game: Option<Running>,
    library: Library,
    menus: Menus,
    jobs: Vec<Job>,
    gui: Option<Gui>,
    // backend must be declared (and therefore dropped) before window, Rust
    // drops struct fields in declaration order, and the GL surface's Drop
    // calls eglDestroySurface, which on Wayland does a protocol round-trip
    // against the window's wl_surface.
    backend: Option<Backend>,
    window: Option<Window>,
    keyboard: Keyboard,
    gamepads: gamepad::Gamepads,
    /// a controller's gyroscope, for aiming with it.
    gyro: gyro::Gyro,
    /// the controller whose buttons the game's prompts show, none for the
    /// keyboard and mouse: whichever the player used last.
    prompt_faces: Option<gamepad::Faces>,
    /// what the running game's prompts were last made for, none when they
    /// are due again: a new game, or bindings changed.
    prompts_shown: Option<Option<gamepad::Faces>>,
    audio: Option<audio::Audio>,
    /// the window's size, times the console's.
    scale: u32,
    /// how the screens are arranged.
    layout: Screens,
    /// how wide the top screen shows.
    top_shape: settings::TopShape,
    /// the left button is down, and where the pointer is, in window pixels.
    mouse_down: bool,
    modifiers: winit::keyboard::ModifiersState,
    /// where the pointer was when the right button went down, which tilts
    /// the console while held.
    tilting_from: Option<(f32, f32)>,
    /// the mouse aims in Kid Icarus: Uprising, F6 turns it on and off: the
    /// pointer is held in the window, its movement moves the reticle, the
    /// bottom screen is held touched in its middle and the left button
    /// fires.
    aim_captured: bool,
    /// the mouse was let go for the pause menu on the top screen, and is
    /// held again once it is gone.
    released_for_pause: bool,
    cursor: (f32, f32),
    last_title_update: Instant,
    /// when the next frame is due. frames run to a schedule rather than one
    /// after another, so that a slow one is made up by those after it and
    /// the sound, made as the console runs, keeps up.
    next_frame: Instant,
    /// frames in a row not shown while catching up.
    skipped: u32,
    /// the pictures shown a second, the host's in High Refresh Mode.
    shown: FrameRate,
    /// how long emulating and showing frames took since the title was last
    /// updated, for the frame rate's log.
    spent: Spent,
    /// stopped with F1, without the menu.
    paused: bool,
    /// Tab is held down, for fast forward.
    fast_forward_key: bool,
    /// the game runs as fast as it can, its sound left out.
    fast_forwarding: bool,
    /// when fast forward shows its next frame, on a grid at the console's
    /// frame rate.
    next_shown: Instant,
    /// the display's own refresh interval, before the High Refresh Mode cap.
    display_step: Duration,
    /// the refresh interval High Refresh Mode shows a picture at, the
    /// display's own or the cap's, whichever is slower.
    host_step: Duration,
    /// when High Refresh Mode shows its next picture.
    next_host: Instant,
    /// the host time the picture clock counts from, so a picture in
    /// between can be drawn at the fraction of the way it belongs at.
    host_epoch: Instant,
    /// the host time the last console frame started at, and the one before
    /// it: the two a picture in between is drawn between.
    frame_host: Duration,
    older_frame_host: Duration,
    /// the host time the next picture is drawn at, a console frame behind
    /// the frames' own so both of the last two are there to draw between.
    shown_host: Duration,
    /// the High Refresh Mode setting last put to the running game.
    high_refresh_applied: bool,
    stop: bool,
    /// what the running game's cameras were given to see, to give them
    /// another only when the settings change it.
    camera: Option<(CameraSource, Option<PathBuf>)>,
    /// the room this program holds open, and the one it is in.
    room_server: Option<zakuro_core::room::Server>,
    room: Option<zakuro_core::room::Connection>,
    /// Download Play as a guest, while in a room with no game running.
    dlp_guest: Option<zakuro_core::download_play::Guest>,
    /// the player's own copy of the child joined for, played in place of
    /// one sent.
    dlp_own: Option<PathBuf>,
    /// the network the next game joins as it starts, the host's of a
    /// Download Play child.
    wireless_reboot: Option<[u8; 16]>,
    /// the window was maximized when it went full screen, which leaving
    /// full screen brings back.
    maximized_before_full_screen: bool,
    /// the layout or the scale changed while the window was full screen,
    /// which kept its size, so leaving full screen fits the window to them.
    resize_after_full_screen: bool,
}

/// how long each step of the loop took, its mean and its longest.
#[derive(Default)]
struct Spent {
    emulating: Times,
    showing: Times,
}

#[derive(Default)]
struct Times {
    total: Duration,
    longest: Duration,
    count: u32,
}

impl Times {
    fn add(&mut self, time: Duration) {
        self.total += time;
        self.longest = self.longest.max(time);
        self.count += 1;
    }
}

impl std::fmt::Display for Times {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mean = self.total.as_secs_f64() * 1e3 / self.count.max(1) as f64;
        write!(f, "{mean:.1} ms, at most {:.1}", self.longest.as_secs_f64() * 1e3)
    }
}

/// how many of something go by a second, counted between reads, the game's
/// own frames and the host's pictures kept apart.
struct FrameRate {
    /// counted since the last read.
    counted: u32,
    /// when the count started.
    since: Instant,
    /// what the last read gave a second.
    rate: f32,
}

impl FrameRate {
    fn new() -> FrameRate {
        FrameRate { counted: 0, since: Instant::now(), rate: 0.0 }
    }

    /// one more went by.
    fn count(&mut self) {
        self.counted += 1;
    }

    /// what went by a second since the last read, once half a second has
    /// gone by, and the last rate before that.
    fn read(&mut self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.since);
        if elapsed < Duration::from_millis(500) {
            return self.rate;
        }
        self.rate = self.counted as f32 / elapsed.as_secs_f32();
        self.counted = 0;
        self.since = now;
        self.rate
    }

    /// the rate the last read gave.
    fn rate(&self) -> f32 {
        self.rate
    }
}

/// one 3DS frame, about 16.71 ms.
const FRAME_TIME: Duration =
    Duration::from_nanos(zakuro_core::CYCLES_PER_FRAME * 1_000_000_000 / zakuro_core::kernel::thread::CPU_CLOCK_HZ);
/// how far behind the schedule may fall before it starts over instead of
/// running fast to catch up, after a pause say.
const CATCH_UP_LIMIT: Duration = Duration::from_millis(200);
/// frames that may go unshown in a row while catching up.
const MAX_SKIPPED: u32 = 4;

/// Winit reports content motion; menus take viewport motion, down/right positive.
fn menu_wheel_delta(delta: MouseScrollDelta, shift: bool) -> [f32; 2] {
    let (x, y) = match delta {
        MouseScrollDelta::LineDelta(x, y) => (x, y),
        MouseScrollDelta::PixelDelta(position) => (position.x as f32 / 40.0, position.y as f32 / 40.0),
    };
    if shift { [-x - y, 0.0] } else { [-x, -y] }
}

/// whether a frame goes unshown, the game running behind the schedule with
/// a presenter that waits for the display, and fewer than MAX_SKIPPED
/// unshown before it.
fn skips_showing(playing: bool, behind: bool, waits: bool, skipped: u32) -> bool {
    playing && behind && waits && skipped < MAX_SKIPPED
}

/// whether a picture goes in between the title's own, one on every refresh
/// of the display, rather than the title's own at the console's rate. fast
/// forward shows real frames only, so no GPU time goes on pictures nobody
/// needs.
fn draws_in_between(high_refresh: bool, fast_forwarding: bool) -> bool {
    high_refresh && !fast_forwarding
}

/// when the next picture is due, a display refresh after the one scheduled,
/// or a refresh on from now when that has already passed: the refreshes a
/// slow picture missed go unshown rather than pile up.
fn next_host_time(scheduled: Instant, step: Duration, now: Instant) -> Instant {
    let next = scheduled + step;
    if next <= now { now + step } else { next }
}

/// the display's refresh interval, one picture a refresh in High Refresh
/// Mode, 60 a second when it can't be read.
fn monitor_step(window: &Window) -> Duration {
    let millihertz = window
        .current_monitor()
        .and_then(|monitor| monitor.refresh_rate_millihertz())
        .filter(|&millihertz| millihertz > 0)
        .unwrap_or(60_000);
    Duration::from_nanos((1_000_000_000_000 / u64::from(millihertz)).max(1))
}

/// what the player is told when the 3D can't be drawn on the GPU.
const GPU_FALLBACK: &str = "The 3D can't be drawn on the GPU, as Vulkan isn't working on this computer, so it's drawn in software. \
Games run much slower that way, and texture packs are left out. Updating the graphics driver usually fixes it; the log says \
what went wrong. To draw in software without this message, turn off \"Draw the 3D on the GPU\" in Settings, Graphics.";

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let size = self.window_size();
        let attributes = Window::default_attributes().with_title("Zakuro").with_inner_size(size);
        let renderer = self.options.renderer.unwrap_or(match self.settings.renderer {
            Renderer::Vulkan => RendererKind::Vulkan,
            Renderer::OpenGl => RendererKind::OpenGl,
        });

        let backend = Backend::create(event_loop, attributes.clone(), renderer).or_else(|error| {
            if renderer != RendererKind::Vulkan {
                return Err(error);
            }
            // a machine without a working Vulkan driver still gets a window
            log::warn!("could not start the Vulkan backend, {error}, presenting with OpenGL instead");
            Backend::create(event_loop, attributes, RendererKind::OpenGl)
        });
        match backend {
            Ok((window, mut backend)) => {
                log::info!("presenting with the {} backend", backend.name());
                backend.set_layout(self.layout.screens());
                backend.set_top_width(self.top_width());
                backend.set_scaling(self.settings.filter.screens(), self.settings.integer_scale);
                backend.set_stereo(self.settings.stereo());
                self.gui = Some(Gui::new(&window));
                self.display_step = monitor_step(&window);
                self.host_step = self.settings.high_refresh_cap.step(self.display_step);
                self.next_host = Instant::now();
                self.window = Some(window);
                self.backend = Some(backend);
                // the game the command line names starts on the device the
                // presenter may lend the renderer, once there is one
                self.start();
            }
            Err(error) => {
                eprintln!("zakuro: could not start the {renderer:?} backend: {error}");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Tab is fast forward while a game runs, not a key for moving the
        // interface's focus around
        let (tab, repeat) = match &event {
            WindowEvent::KeyboardInput { event, .. } => (event.physical_key == PhysicalKey::Code(KeyCode::Tab), event.repeat),
            _ => (false, false),
        };
        let in_game = self.game.as_ref().is_some_and(|game| game.system.keyboard_request().is_none())
            && !self.menus.menu_open
            && !self.menus.settings_open;
        // a Tab held down keeps repeating over a message too
        let withheld = tab && (in_game || (repeat && self.game.is_some()));
        let consumed = match (&mut self.gui, &self.window) {
            (Some(gui), Some(window)) if !withheld => gui.event(window, &event),
            _ => false,
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            // keys let go while another window has the keyboard never come
            WindowEvent::Focused(false) => {
                self.fast_forward_key = false;
                self.released_for_pause = false;
                self.set_aim_captured(false);
                self.keyboard.release();
                self.mouse_down = false;
                self.modifiers = winit::keyboard::ModifiersState::empty();
                if let Some(aim_mouse) = self.game.as_mut().and_then(|game| game.system.aim_mouse.as_mut()) {
                    aim_mouse.cancel_menu_touch();
                    aim_mouse.menu_touch(0, false);
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(mouse) = self.game.as_mut().and_then(|game| game.system.aim_mouse.as_mut()) {
                    mouse.cancel_menu_touch();
                }
                let top_width = self.top_width();
                if let Some(backend) = &mut self.backend {
                    backend.resize(size.width, size.height);
                    backend.set_top_width(top_width);
                }
            }
            // another display can refresh at another rate, which High
            // Refresh Mode follows
            WindowEvent::Moved(_) | WindowEvent::ScaleFactorChanged { .. } => self.measure_host_step(),
            WindowEvent::KeyboardInput { event, .. } => self.key(event, consumed),
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::MouseWheel { delta, .. } if !self.aim_captured && !self.pointer_taken(consumed) => {
                self.touch();
                let delta = menu_wheel_delta(delta, self.modifiers.shift_key());
                if let Some(game) = &mut self.game {
                    if let Some(mouse) = &mut game.system.aim_mouse {
                        mouse.scroll_menu(game.system.frames, delta);
                    }
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button, .. }
                if self.capture_binding(bindings::Source::Mouse(button)) => {}
            WindowEvent::MouseInput { state, button, .. } if self.aim_captured => {
                let pressed = state == ElementState::Pressed;
                if self.keyboard.acting() {
                    let started = self.keyboard.source(bindings::Source::Mouse(button), pressed);
                    self.do_actions(started);
                    self.prompt_faces = None;
                } else if button == MouseButton::Left {
                    self.keyboard.mouse_button(zakuro_core::services::hid::PadState::L, pressed);
                    self.prompt_faces = None;
                }
            }
            WindowEvent::CursorMoved { .. } if self.aim_captured => {}
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    let pressed = state == ElementState::Pressed && !self.pointer_taken(consumed);
                    self.mouse_down = pressed;
                    self.touch();
                }
                if button == MouseButton::Right {
                    let pressed = state == ElementState::Pressed && !self.pointer_taken(consumed);
                    self.tilting_from = pressed.then_some(self.cursor);
                    self.tilt();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
                self.touch();
                self.tilt();
            }
            WindowEvent::RedrawRequested => self.step(event_loop),
            _ => {}
        }
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let (true, DeviceEvent::MouseMotion { delta }) = (self.aim_captured, event) {
            let sensitivity = self.settings.pc.aim.mouse_sensitivity;
            if let Some(aim_mouse) = self.game.as_mut().and_then(|game| game.system.aim_mouse.as_mut()) {
                aim_mouse.add(delta.0 as f32 * sensitivity, delta.1 as f32 * sensitivity);
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl App {
    /// the console's settings for a game, from the command line and the
    /// settings.
    fn config(&self) -> Config {
        let interpret = self.options.interpreter || !self.settings.recompiled;
        Config {
            new3ds: self.options.new3ds,
            data_dir: self.data_dir.clone(),
            language: self.settings.language.code(),
            recompiled: self.options.recompiled.clone().filter(|_| !interpret).map(Into::into),
            linked: self.linked.filter(|_| !interpret),
            find_recompiled: !interpret,
            hardware_renderer: self.options.hardware_rasterizer.unwrap_or(self.settings.hardware_rasterizer),
            texture_packs: self.settings.texture_packs,
            interpolate: self.settings.high_refresh_mode,
            enhancements: self
                .settings
                .enhancements
                .iter()
                .filter_map(|(id, names)| Some((u64::from_str_radix(id, 16).ok()?, names.clone())))
                .collect(),
            resolution: self.settings.resolution,
            device: self.backend.as_ref().and_then(Backend::shared_device),
            username: self.nickname(),
            console_id: self.settings.console_id,
            slider_3d: self.settings.slider_3d(),
            ..Config::default()
        }
    }

    /// starts what the command line asks for, the test pattern or a game.
    fn start(&mut self) {
        if self.options.test_pattern {
            // the test pattern has no CPU program to run, paused keeps just
            // presenting what is there
            let system = paint_test_pattern(self.config());
            self.game = Some(Running::new(system, PathBuf::new(), "test pattern".to_owned()));
            self.paused = true;
        } else if let Some(rom) = self.options.rom.clone() {
            if let Err(error) = self.play(Path::new(&rom)) {
                eprintln!("zakuro: {error}");
                std::process::exit(1);
            }
        }
        if self.options.profile {
            if let Some(game) = &mut self.game {
                game.system.enable_profiler();
            }
        }
    }

    /// starts the game at path, in place of any other.
    fn play(&mut self, path: &Path) -> Result<(), String> {
        // the old game and what it holds on the GPU go first, its sound too
        self.game = None;
        if let Some(audio) = &self.audio {
            audio.clear();
        }
        // a Download Play guest goes with the game
        self.dlp_guest = None;
        let mut config = self.config();
        config.wireless_reboot = self.wireless_reboot.take();
        // the update, DLC and Download Play child found with the game in
        // the library
        if let Some(game) = self.library.games.iter().find(|game| game.path == path) {
            config.update = game.update.as_ref().map(|update| update.path.clone());
            config.dlc = game.dlc.iter().map(|dlc| dlc.path.clone()).collect();
            config.dlp_child = game.dlp_child.as_ref().map(|child| child.path.clone());
        } else if let Some((update, dlc, dlp_child)) = library::addons_beside(path) {
            // one from the command line starts before the library's scan is
            // in, or isn't in its folder: what's next to it goes with it
            config.update = update.map(|update| update.path);
            config.dlc = dlc.into_iter().map(|dlc| dlc.path).collect();
            config.dlp_child = dlp_child.map(|child| child.path);
        }
        // a recording starts the clock at a known time, for playing it back
        let record = std::env::var_os("ZAKURO_RECORD").map(PathBuf::from);
        let clock = zakuro_core::memory::config::host_clock();
        if record.is_some() {
            config.clock = Some(clock);
        }
        // and plays back from the clock it was made at
        let replay = match std::env::var_os("ZAKURO_REPLAY").map(PathBuf::from) {
            Some(file) => {
                let replay = zakuro_core::replay::Replay::open(&file).map_err(|error| format!("could not read {}, {error}", file.display()))?;
                log::info!("playing back the inputs in {}", file.display());
                config.clock = Some(replay.clock());
                Some(replay)
            }
            None => None,
        };
        let mut system = loader::load(path, config).map_err(|error| format!("could not open {}, {error}", path.display()))?;
        // the mouse aims through stops the interpreter makes, which the
        // recompiled code doesn't: the functions they are in are left to it
        if system.title.as_ref().is_some_and(|title| title.program_id() == zakuro_core::aim_mouse::PROGRAM_ID) {
            if let Some(library) = &mut system.recompiled {
                if !zakuro_core::aim_mouse::PLACES.iter().all(|&address| library.interpret_at(address) > 0) {
                    log::warn!("the recompiled code lacks a function mouse aiming stops in, interpreting all of it");
                    system.recompiled = None;
                }
            }
            // the HUD, reticle and portraits at the PC Preferences' scales
            let hud_scale = self.settings.pc.presentation.hud_scale;
            // the narration drawn on the top screen as well, ZAKURO_NARRATION_TOP=0 leaves it below
            let narration_on_top = std::env::var("ZAKURO_NARRATION_TOP").map_or(true, |value| value != "0");
            let portrait_scale = self.settings.pc.presentation.portrait_scale;
            // and the hearts, item box and power icon, ZAKURO_MIRROR_BOTTOM=0 leaves them below
            let mirror_bottom = std::env::var("ZAKURO_MIRROR_BOTTOM").map_or(true, |value| value != "0");
            // and the pause menu, where the mouse touches it, ZAKURO_PAUSE_TOP=0 leaves it below
            let pause_on_top = std::env::var("ZAKURO_PAUSE_TOP").map_or(true, |value| value != "0");
            // and its options lit by the mouse over them, as on a PC, ZAKURO_MENU_HOVER=0 as a stylus
            let menu_hover = std::env::var("ZAKURO_MENU_HOVER").map_or(true, |value| value != "0");
            let mut aim_mouse = zakuro_core::aim_mouse::AimMouse::new(hud_scale, narration_on_top, portrait_scale, mirror_bottom, pause_on_top, menu_hover);
            // and the Fiend's Cauldron's two screens on the top screen, where
            // the mouse touches them, ZAKURO_CAULDRON_TOP=0 leaves them as they are
            aim_mouse.cauldron_on_top = std::env::var("ZAKURO_CAULDRON_TOP").map_or(true, |value| value != "0");
            aim_mouse.menus_on_top = std::env::var("ZAKURO_MENUS_TOP").map_or(true, |value| value != "0");
            system.aim_mouse = Some(aim_mouse);
            if let Some(mouse) = system.aim_mouse.as_mut() {
                mouse.set_editor_overrides(self.layout_overrides.clone());
                mouse.set_pause_rules(self.pause_rules);
                mouse.set_dialog_rules(self.dialog_rules);
                mouse.set_cauldron_rules(self.cauldron_rules);
                if self.options.menu_editor {
                    mouse.set_editor_navigation(self.navigation.clone());
                    mouse.set_editor_hidden_layers(self.hidden.clone());
                }
            }
            // at 60 FPS the game has half a 3DS's time for each frame, which
            // its busiest scenes need more than: ZAKURO_OVERCLOCK times the
            // instructions in the same time, 1 as the console
            system.cpu.overclock = std::env::var("ZAKURO_OVERCLOCK").ok().and_then(|value| value.parse().ok()).unwrap_or(2).max(1);
            log::info!("mouse aiming available, F6 captures the mouse, HUD drawn at {hud_scale}, CPU at {}x", system.cpu.overclock);
        }
        let outdated = system.recompiled.as_ref().is_some_and(|code| code.outdated());
        // the settings ask for the 3D on the GPU, but Vulkan didn't start:
        // only the log said so, while the game ran slowly
        let in_software = system.config.hardware_renderer && !system.gpu.draws_on_gpu();
        let name = self
            .library
            .games
            .iter()
            .find(|game| game.path == path)
            .map(|game| game.name.clone())
            .or_else(|| Some(library::Smdh::parse(system.title.as_ref()?.exefs_file("icon")?)?.name).filter(|name| !name.is_empty()))
            .or_else(|| path.file_stem().map(|stem| stem.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let mut game = Running::new(system, path.to_owned(), name);
        game.replay = replay;
        if let Some(record) = record {
            match zakuro_core::replay::Recorder::create(&record, clock) {
                Ok(recorder) => {
                    log::info!("recording the inputs to {}", record.display());
                    game.recorder = Some(recorder);
                }
                Err(error) => log::warn!("could not record to {}, {error}", record.display()),
            }
        }
        if outdated {
            self.menus.message = Some(format!(
                "{} was recompiled by an older version of Zakuro. Recompile it again from the library to get the newest improvements.",
                game.name
            ));
        } else if in_software {
            self.menus.message = Some(GPU_FALLBACK.to_owned());
        }
        self.game = Some(game);
        self.prompts_shown = None;
        // the new game's cameras see what the settings say, and it plays
        // in the room the player is in
        self.camera = None;
        self.apply_camera();
        self.link_room();
        self.menus.menu_open = false;
        self.keyboard.release();
        self.apply_bindings();
        self.paused = false;
        self.next_frame = Instant::now();
        if let Some(window) = &self.window {
            self.display_step = monitor_step(window);
            self.host_step = self.settings.high_refresh_cap.step(self.display_step);
        }
        self.high_refresh_applied = self.settings.high_refresh_mode;
        self.reset_host_pace();
        Ok(())
    }

    fn back_to_library(&mut self) {
        self.game = None;
        if let Some(room) = &self.room {
            room.playing("");
        }
        if let Some(audio) = &self.audio {
            audio.clear();
        }
        self.menus.menu_open = false;
        self.keyboard.release();
    }

    fn key(&mut self, event: KeyEvent, consumed: bool) {
        let pressed = event.state == ElementState::Pressed;
        let PhysicalKey::Code(code) = event.physical_key else { return };
        // Escape leaves a binding waiting for an input as it was
        if pressed && code == KeyCode::Escape && (self.menus.rebinding_pad.take().is_some() || self.menus.rebinding_action.take().is_some()) {
            return;
        }
        if pressed && !event.repeat && self.capture_binding(bindings::Source::Key(code)) {
            return;
        }
        // a binding waiting for a key takes it, Escape leaves it as it was
        if let (true, Some(index)) = (pressed, self.menus.rebinding) {
            self.menus.rebinding = None;
            if code != KeyCode::Escape {
                if let Some((_, key)) = self.settings.keys.all_mut().into_iter().nth(index) {
                    *key = code;
                }
                self.apply_settings();
            }
            return;
        }
        let tab_bound = match self.keyboard.acting() {
            true => self.settings.pc.bindings.binds(bindings::Source::Key(KeyCode::Tab)),
            false => self.settings.keys.binds(KeyCode::Tab),
        };
        if code == KeyCode::Tab && !event.repeat && !tab_bound {
            // fast forward while it is held; the editor owns the keyboard
            self.fast_forward_key = pressed && self.game.is_some() && !self.menus.editor_open;
        }
        if pressed && !event.repeat {
            match code {
                KeyCode::F6 if !self.menus.editor_open && self.game.as_ref().is_some_and(|game| game.system.aim_mouse.is_some()) => {
                    self.released_for_pause = false;
                    self.set_aim_captured(!self.aim_captured);
                    return;
                }
                KeyCode::F2 if self.options.menu_editor => {
                    self.released_for_pause = false;
                    self.set_aim_captured(false);
                    self.menus.editor_open = !self.menus.editor_open;
                    self.menus.toast = Some((format!("Layout editor {}", if self.menus.editor_open { "on" } else { "off" }), Instant::now()));
                    self.keyboard.release();
                    return;
                }
                // the Frontend Menu: F12 always, Escape and Home too for a
                // title whose Escape is the console's; Kid Icarus's Escape is
                // its own Pause and Back
                KeyCode::F12 if self.game.is_some() => {
                    self.toggle_frontend_menu();
                    return;
                }
                KeyCode::Escape if self.game.is_some() && !self.keyboard.acting() => {
                    self.toggle_frontend_menu();
                    return;
                }
                KeyCode::F1 => {
                    self.paused = !self.paused;
                    log::info!("{}", if self.paused { "paused" } else { "resumed" });
                }
                KeyCode::F9 => {
                    self.settings.layout = self.settings.layout.next();
                    self.save_settings();
                    self.apply_layout();
                }
                KeyCode::F10 if self.settings.layout.other_screen() != self.settings.layout => {
                    self.settings.layout = self.settings.layout.other_screen();
                    self.save_settings();
                    self.apply_layout();
                }
                KeyCode::F11 => self.toggle_fullscreen(),
                KeyCode::F5 if self.game.is_some() => self.save_state(),
                KeyCode::F8 if self.game.is_some() => self.load_state(),
                _ => {}
            }
        }
        let to_game = self.game.is_some()
            && !self.menus.menu_open
            && !self.menus.settings_open
            && !self.menus.editor_open
            && !consumed
            && !self.gui.as_ref().is_some_and(Gui::wants_keyboard);
        if self.keyboard.acting() {
            // letting go always gets through, so that no action stays held
            if (to_game && !event.repeat) || !pressed {
                let started = self.keyboard.key(event.physical_key, pressed);
                self.do_actions(started);
            }
            if to_game && pressed {
                self.prompt_faces = None;
            }
            return;
        }
        // the power key uses a power in a battle, and anywhere else does what
        // else it is bound to, if anything. held, it uses no more than the one;
        // the editor owns the keyboard, so typing there cannot use a power.
        if pressed && code == self.settings.keys.power && !self.menus.editor_open {
            if let Some(game) = &mut self.game {
                let frame = game.system.frames;
                if let Some(aim_mouse) = game.system.aim_mouse.as_mut().filter(|aim_mouse| aim_mouse.in_battle(frame)) {
                    if !event.repeat {
                        aim_mouse.use_power(frame);
                    }
                    return;
                }
            }
        }
        if to_game && pressed && !event.repeat {
            let exit = if code == self.settings.keys.vehicle_ability {
                Some(false)
            } else if code == self.settings.keys.vehicle_exit {
                Some(true)
            } else {
                None
            };
            if let (Some(exit), Some(game)) = (exit, &mut self.game) {
                let frame = game.system.frames;
                if game.system.aim_mouse.as_mut().is_some_and(|mouse| mouse.use_vehicle(frame, exit)) {
                    self.prompt_faces = None;
                    return;
                }
            }
        }
        // letting go always gets through, so that no button stays held
        if to_game || !pressed {
            self.keyboard.key(event.physical_key, pressed);
        }
        if to_game && pressed {
            self.prompt_faces = None;
        }
    }

    /// opens or closes the Frontend Menu over the game.
    fn toggle_frontend_menu(&mut self) {
        self.released_for_pause = false;
        self.set_aim_captured(false);
        self.menus.menu_open = !self.menus.menu_open;
        self.keyboard.release();
    }

    /// does what the actions a press started do outside the console's
    /// buttons: a power, or a vehicle's ability or exit.
    fn do_actions(&mut self, started: Vec<bindings::Action>) {
        let Some(mouse) = self.game.as_mut().and_then(|game| {
            let frame = game.system.frames;
            game.system.aim_mouse.as_mut().map(|mouse| (mouse, frame))
        }) else {
            return;
        };
        let (mouse, frame) = mouse;
        for action in started {
            match action {
                bindings::Action::UsePower => mouse.use_power(frame),
                bindings::Action::VehicleAbility => _ = mouse.use_vehicle(frame, false),
                bindings::Action::ExitVehicle => _ = mouse.use_vehicle(frame, true),
                _ => {}
            }
        }
    }

    /// gives a Kid Icarus action waiting for an input this one, when it is
    /// the device's, asking first when another action already uses it.
    fn capture_binding(&mut self, source: bindings::Source) -> bool {
        let Some((action, device, slot)) = self.menus.rebinding_action else { return false };
        if source.device() != device {
            return false;
        }
        self.menus.rebinding_action = None;
        let conflicts = self.settings.pc.bindings.conflicts(action, source);
        if conflicts.is_empty() {
            if let Err(error) = self.settings.pc.bindings.assign(action, slot, source, bindings::Resolve::Replace) {
                log::warn!("binding {} refused: {error:?}", action.name());
            }
            self.apply_settings();
        } else {
            self.menus.binding_conflict = Some((action, slot, source, conflicts));
        }
        true
    }

    /// takes in the controllers' buttons, a binding waiting for one takes
    /// it, and Home opens and closes the menu over the game.
    fn poll_gamepads(&mut self) {
        if std::mem::take(&mut self.menus.calibrate_gyro) {
            self.gyro.calibrate();
        }
        self.gyro.update();
        self.menus.gyro_status = match (self.gyro.status(), self.gyro.controller()) {
            (gyro::Status::Ready, Some(name)) => format!("Gyroscope: {name}"),
            (gyro::Status::Calibrating, _) => "Gyroscope: calibrating, keep the controller still".to_owned(),
            (gyro::Status::NoGyroscope, _) => "Gyro unavailable for this controller".to_owned(),
            (gyro::Status::NoController, _) => "No controller connected".to_owned(),
            (gyro::Status::Unavailable, _) | (gyro::Status::Ready, None) => "Controller gyroscopes can't be read on this system".to_owned(),
        };
        for (button, pressed) in self.gamepads.poll() {
            if pressed && button == gilrs::Button::Mode && self.menus.rebinding_action.take().is_some() {
                continue;
            }
            if pressed && self.capture_binding(bindings::Source::Pad(button)) {
                continue;
            }
            if let (true, Some(index)) = (pressed, self.menus.rebinding_pad) {
                self.menus.rebinding_pad = None;
                if let Some((_, bound)) = self.settings.pad.all_mut().into_iter().nth(index) {
                    *bound = button;
                }
                self.apply_settings();
            } else if pressed && button == gilrs::Button::Mode && self.game.is_some() {
                self.toggle_frontend_menu();
            } else if self.keyboard.acting() {
                let to_game = !self.menus.menu_open && !self.menus.settings_open && !self.menus.editor_open;
                if to_game || !pressed {
                    let started = self.keyboard.source(bindings::Source::Pad(button), pressed);
                    self.do_actions(started);
                }
            } else if pressed && !self.menus.menu_open && !self.menus.settings_open && !self.menus.editor_open {
                let exit = if button == self.settings.pad.vehicle_ability {
                    Some(false)
                } else if button == self.settings.pad.vehicle_exit {
                    Some(true)
                } else {
                    None
                };
                if let (Some(exit), Some(game)) = (exit, &mut self.game) {
                    let frame = game.system.frames;
                    if let Some(mouse) = &mut game.system.aim_mouse {
                        mouse.use_vehicle(frame, exit);
                    }
                }
            }
        }
        if self.gamepads.disconnected() {
            self.keyboard.release_where(|source| matches!(source, bindings::Source::Pad(_)));
        }
        if let Some(faces) = self.gamepads.used() {
            self.prompt_faces = Some(faces);
        }
    }

    /// shows the game's button prompts as the keys, mouse or controller the
    /// player used last, bound as the settings have them, when that changed.
    fn update_prompts(&mut self) {
        let Some(game) = &mut self.game else { return };
        if self.prompts_shown == Some(self.prompt_faces) {
            return;
        }
        let bindings = &self.settings.pc.bindings;
        let prompts = match (self.prompt_faces, self.keyboard.acting()) {
            // Kid Icarus shows the inputs on its actions
            (Some(faces), true) => bindings.prompts(bindings::Device::Controller, |source| match source {
                bindings::Source::Pad(button) => gamepad::label(button, faces),
                other => input::label(other),
            }),
            (None, true) => bindings.prompts(bindings::Device::KeyboardMouse, input::label),
            (Some(faces), false) => self.gamepads.prompts(faces),
            (None, false) => input::prompts(&self.settings.keys),
        };
        game.system.set_prompts(prompts);
        self.prompts_shown = Some(self.prompt_faces);
    }

    /// holds the pointer for aiming or lets it go back to touching.
    fn set_aim_captured(&mut self, captured: bool) {
        if captured == self.aim_captured {
            return;
        }
        let Some(window) = &self.window else { return };
        if captured {
            if window.set_cursor_grab(CursorGrabMode::Locked).or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined)).is_err() {
                log::warn!("could not hold the pointer in the window");
                return;
            }
            window.set_cursor_visible(false);
            // held still, the touch moves nothing, but the game keeps the
            // reticle where the mouse put it instead of easing it back
            self.keyboard.touch(Some((160, 120)));
        } else {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            window.set_cursor_visible(true);
            self.keyboard.touch(None);
            self.keyboard.mouse_button(zakuro_core::services::hid::PadState::L, false);
            // a mouse button held into the release lets go of its actions
            // now, as its own release no longer reaches them
            self.keyboard.release_where(|source| matches!(source, bindings::Source::Mouse(_)));
        }
        self.aim_captured = captured;
        self.mouse_down = false;
        if let Some(aim_mouse) = self.game.as_mut().and_then(|game| game.system.aim_mouse.as_mut()) {
            aim_mouse.cancel_menu_touch();
            aim_mouse.menu_touch(0, false);
            aim_mouse.pending = [0.0; 2];
            aim_mouse.captured = captured;
        }
        log::info!("mouse aiming {}", if captured { "on, F6 lets go" } else { "off" });
    }

    /// whether Kid Icarus: Uprising shows its pause menu, or the Fiend's
    /// Cauldron, on the top screen, for the mouse to touch there.
    fn pause_menu_shown(&self) -> bool {
        self.game.as_ref().is_some_and(|game| {
            let frames = game.system.frames;
            game.system.aim_mouse.as_ref().is_some_and(|aim_mouse| aim_mouse.pause_menu_shown(frames) || aim_mouse.cauldron_on_top_shown(frames) || aim_mouse.menus_on_top_shown(frames))
        })
    }

    /// lets the mouse go while the pause menu is on the top screen, for
    /// touching it there, and holds it again once the menu is gone, when it
    /// was let go for it.
    fn follow_pause_menu(&mut self) {
        if self.menus.editor_open {
            if self.aim_captured { self.set_aim_captured(false); }
            return;
        }
        let shown = self.pause_menu_shown();
        if shown && self.aim_captured {
            self.set_aim_captured(false);
            self.released_for_pause = true;
        } else if !shown && self.released_for_pause {
            self.released_for_pause = false;
            // the click that closed it is not still touching
            self.mouse_down = false;
            self.set_aim_captured(true);
        }
    }

    /// whether the pointer is busy with something other than the game.
    fn pointer_taken(&self, consumed: bool) -> bool {
        consumed || self.game.is_none() || self.menus.menu_open || self.menus.editor_open || self.gui.as_ref().is_some_and(Gui::wants_pointer)
    }

    /// touches the bottom screen while the button is down over it, and
    /// tells the game's menus where over it the mouse is, for them to light
    /// what is under it.
    fn touch(&mut self) {
        if self.pointer_taken(false) {
            if let Some(mouse) = self.game.as_mut().and_then(|game| game.system.aim_mouse.as_mut()) {
                mouse.cancel_menu_touch();
                mouse.clear_menus_top_pointer();
                if !self.mouse_down { mouse.menu_touch(0, false); }
            }
            self.keyboard.touch(None);
            return;
        }
        let Some(window) = &self.window else { return };
        let size = window.inner_size();
        let (top, bottom) = layout(size.width, size.height, self.layout.screens(), self.top_width(), self.settings.integer_scale);
        if top.is_none() {
            if let Some(mouse) = self.game.as_mut().and_then(|game| game.system.aim_mouse.as_mut()) {
                mouse.clear_menus_top_pointer();
            }
        }
        let top_pointer = top.and_then(|top| {
            let x = (self.cursor.0 - top.x) / top.width * 400.0;
            let y = (self.cursor.1 - top.y) / top.height * 240.0;
            let game = self.game.as_mut()?;
            let frames = game.system.frames;
            let mouse = game.system.aim_mouse.as_mut()?;
            let dragging = self.mouse_down && mouse.menus_top_dragging();
            if !dragging && (!(0.0..400.0).contains(&x) || !(0.0..240.0).contains(&y)) {
                mouse.clear_menus_top_pointer();
                return None;
            }
            mouse.top_screen_touch(frames, x, y, self.mouse_down)
        });
        let top_dragging = self.mouse_down && self.game.as_ref().and_then(|game| game.system.aim_mouse.as_ref()).is_some_and(|mouse| mouse.menus_top_dragging());
        let pointer = top_pointer.or_else(|| {
            if top_dragging { return None; }
            let bottom = bottom?;
            let x = (self.cursor.0 - bottom.x) / bottom.width * 320.0;
            let y = (self.cursor.1 - bottom.y) / bottom.height * 240.0;
            ((0.0..320.0).contains(&x) && (0.0..240.0).contains(&y)).then_some((x, y))
        });
        let pointer = pointer.filter(|_| !self.pointer_taken(false));
        let touch = if let Some(game) = &mut self.game {
            if let Some(aim_mouse) = &mut game.system.aim_mouse {
                aim_mouse.hover(pointer);
                aim_mouse.menu_touch(game.system.frames, self.mouse_down)
            } else {
                pointer.filter(|_| self.mouse_down)
            }
        } else {
            None
        };
        self.keyboard.touch(touch.map(|(x, y)| (x.clamp(0.0, 319.0) as u16, y.clamp(0.0, 239.0) as u16)));
    }

    /// tilts the console while the right button is held, toward where the
    /// pointer went and by how far, as Citra's motion emulation does,
    /// which titles were tried with. letting go stands it upright again.
    fn tilt(&mut self) {
        // radians a pixel, and the furthest it goes, a quarter turn
        const PER_PIXEL: f32 = 0.01;
        let tilt = match (self.tilting_from, &self.window) {
            (Some((x, y)), Some(window)) => {
                let scale = window.scale_factor() as f32;
                let (dx, dy) = ((self.cursor.0 - x) / scale, (self.cursor.1 - y) / scale);
                let distance = dx.hypot(dy);
                let angle = (distance * PER_PIXEL).min(std::f32::consts::FRAC_PI_2);
                if distance > 0.0 { [dx / distance * angle, dy / distance * angle] } else { [0.0; 2] }
            }
            _ => [0.0; 2],
        };
        self.keyboard.tilt(tilt);
    }

    fn toggle_fullscreen(&mut self) {
        if let Some(window) = &self.window {
            let full = window.fullscreen().is_some();
            if !full {
                self.maximized_before_full_screen = window.is_maximized();
                self.resize_after_full_screen = false;
            }
            window.set_fullscreen((!full).then_some(Fullscreen::Borderless(None)));
            // the layout may have changed meanwhile, which full screen kept
            // the window's size through. a window maximized before comes
            // back maximized, resizing it would take it out of that
            if full && self.resize_after_full_screen && !self.maximized_before_full_screen {
                let _ = window.request_inner_size(self.window_size());
            }
        }
    }

    fn apply_volume(&self) {
        if let Some(audio) = &self.audio {
            audio.set_volume(if self.settings.mute { 0.0 } else { self.settings.volume });
        }
    }

    /// whether the running game shows a picture on every refresh of the
    /// display.
    fn high_refresh(&self) -> bool {
        self.game.as_ref().is_some_and(|game| game.system.gpu.interpolates())
    }

    /// the host time now, on the clock the console's frames are timed by.
    fn host_time(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.host_epoch)
    }

    /// reads the display's refresh rate again, after moving to another.
    fn measure_host_step(&mut self) {
        let Some(window) = &self.window else { return };
        self.display_step = monitor_step(window);
        self.host_step = self.settings.high_refresh_cap.step(self.display_step);
        self.next_host = Instant::now();
    }

    /// starts the pictures and the frame pair over, a toggle's say.
    fn reset_host_pace(&mut self) {
        self.next_host = Instant::now();
        self.shown_host = self.older_frame_host;
        self.frame_host = Duration::ZERO;
        self.older_frame_host = Duration::ZERO;
    }

    /// saves the settings, telling the player when they could not be.
    fn save_settings(&mut self) {
        if let Err(error) = self.settings.save() {
            self.menus.message = Some(format!("The settings can't be saved: {error}"));
        }
    }

    /// saves the settings and puts to use what can change right away.
    fn apply_settings(&mut self) {
        self.save_settings();
        // a cap change takes effect at once, and the pacer starts over so
        // no picture keeps the old pace
        let host_step = self.settings.high_refresh_cap.step(self.display_step);
        if host_step != self.host_step {
            self.host_step = host_step;
            self.reset_host_pace();
            log::info!("High Refresh Mode cap {}", self.settings.high_refresh_cap.name());
        }
        if self.high_refresh_applied != self.settings.high_refresh_mode {
            self.high_refresh_applied = self.settings.high_refresh_mode;
            self.reset_host_pace();
            if let Some(game) = &mut self.game {
                // a fresh pacer and matcher history, so pictures in between
                // never come from before the switch
                let on = game.system.set_interpolation(self.settings.high_refresh_mode);
                log::info!("High Refresh Mode {}", if on { "on" } else { "off" });
            }
        }
        if let Some(game) = &mut self.game {
            // games read the 3D slider every frame
            let slider = self.settings.slider_3d();
            if game.system.config.slider_3d != slider {
                game.system.set_slider_3d(slider);
            }
            if let Some(mouse) = game.system.aim_mouse.as_mut() {
                let shown = &self.settings.pc.presentation;
                mouse.hud_scale = shown.hud_scale;
                mouse.portrait_scale = shown.portrait_scale;
            }
        }
        self.apply_camera();
        self.keyboard.set_keys(self.settings.keys.clone());
        self.apply_bindings();
        self.gamepads.set_buttons(self.settings.pad.clone());
        self.prompts_shown = None;
        self.apply_volume();
        self.apply_layout();
    }

    /// Kid Icarus takes its input through its Action Bindings; any other
    /// title through the console's buttons.
    fn apply_bindings(&mut self) {
        let kiu = self.game.as_ref().is_some_and(|game| game.system.aim_mouse.is_some());
        self.keyboard.set_bindings(kiu.then(|| self.settings.pc.bindings.clone()));
    }

    /// gives the running game's cameras what the settings say they see,
    /// when it is not what they have.
    fn apply_camera(&mut self) {
        let wanted = (self.settings.camera, self.settings.camera_picture.clone());
        let Some(game) = &mut self.game else { return };
        if self.camera.as_ref() == Some(&wanted) {
            return;
        }
        game.system.camera = match &wanted {
            (CameraSource::Webcam, _) => Some(Box::new(camera::Webcam::default())),
            (CameraSource::Picture, Some(path)) => match camera::picture(path) {
                Ok(picture) => {
                    log::info!("the cameras see {}", path.display());
                    Some(Box::new(picture))
                }
                Err(error) => {
                    self.menus.message = Some(error);
                    None
                }
            },
            _ => None,
        };
        self.camera = Some(wanted);
    }

    /// puts the screens' arrangement and the window scale to use, sizing
    /// the window to them.
    fn apply_layout(&mut self) {
        if let Some(backend) = &mut self.backend {
            backend.set_scaling(self.settings.filter.screens(), self.settings.integer_scale);
            backend.set_stereo(self.settings.stereo());
        }
        let scale = if self.options.scale.is_none() { self.settings.scale.max(1) } else { self.scale };
        if scale != self.scale || self.settings.layout != self.layout || self.settings.top_shape != self.top_shape {
            self.scale = scale;
            self.layout = self.settings.layout;
            self.top_shape = self.settings.top_shape;
            if let Some(backend) = &mut self.backend {
                backend.set_layout(self.layout.screens());
            }
            // a full screen or maximized window keeps its size, resizing
            // it would take it out of that, full screen fits it on leaving
            match &self.window {
                Some(window) if window.fullscreen().is_some() => self.resize_after_full_screen = true,
                Some(window) if !window.is_maximized() => {
                    let _ = window.request_inner_size(self.window_size());
                }
                _ => {}
            }
        }
        let top_width = self.top_width();
        if let Some(backend) = &mut self.backend {
            backend.set_top_width(top_width);
        }
    }

    /// the top screen's width at its height of 240: the shape chosen, or
    /// with the top screen alone fitting the window, the window's, from the
    /// console's 5:3 to 32:9.
    fn top_width(&self) -> f32 {
        if self.top_shape != settings::TopShape::Fit {
            return self.top_shape.width();
        }
        let Some(size) = self.window.as_ref().map(|window| window.inner_size()) else { return 400.0 };
        if self.layout != Screens::TopOnly || size.width == 0 || size.height == 0 {
            return 400.0;
        }
        (240.0 * size.width as f32 / size.height as f32).clamp(400.0, 240.0 * 32.0 / 9.0)
    }

    /// the window size that fits the screens at the chosen scale.
    fn window_size(&self) -> winit::dpi::Size {
        let (width, height) = self.layout.screens().size(self.top_shape.width());
        // scaled by whole numbers, a desktop scaled by 125% say would leave a
        // border, so the window gets whole multiples of the screens' pixels
        if let (true, Some(window)) = (self.settings.integer_scale, &self.window) {
            let times = (self.scale as f64 * window.scale_factor()).round().max(1.0) as u32;
            return winit::dpi::PhysicalSize::new(width * times, height * times).into();
        }
        winit::dpi::LogicalSize::new(width * self.scale, height * self.scale).into()
    }

    /// recompiles the library's game at index, with the computer's C
    /// compiler, or the Zig Zakuro downloaded, or else, when download is
    /// set, with Zig downloaded first. with none of those it asks whether
    /// to download it.
    fn recompile(&mut self, index: usize, download: bool) {
        let Some(game) = self.library.games.get(index) else { return };
        self.jobs.retain(|job| job.program_id != game.program_id || !job.stage().finished());
        if self.jobs.iter().any(|job| job.program_id == game.program_id) {
            return;
        }
        let tools = zig::tools_dir(self.data_dir.as_deref());
        let toolchain = match recomp3ds::compile::check(None) {
            Ok(compiler) => Toolchain::Ready(compiler),
            Err(error) => match tools.as_deref().and_then(zig::installed) {
                Some(compiler) => Toolchain::Ready(compiler),
                None => match tools {
                    Some(tools) if download && zig::download_size().is_some() && zig::can_download() => Toolchain::Download(tools),
                    Some(_) if zig::download_size().is_some() && zig::can_download() => {
                        self.menus.compiler_offer = Some(index);
                        return;
                    }
                    Some(_) if zig::download_size().is_some() => {
                        self.menus.message =
                            Some(format!("Recompiling needs a C compiler, gcc or clang, or curl or wget for Zakuro to download one: {error}."));
                        return;
                    }
                    _ => {
                        self.menus.message = Some(format!("Recompiling needs a C compiler: {error}."));
                        return;
                    }
                },
            },
        };
        let update = game.update.as_ref().map(|update| update.path.as_path());
        self.jobs.push(Job::start(&game.path, update, game.program_id, &game.name, self.data_dir.as_deref(), toolchain));
    }

    /// tells about recompiles as they finish.
    fn poll_jobs(&mut self) {
        let mut finished = Vec::new();
        for job in &mut self.jobs {
            if !job.announced && job.stage().finished() {
                job.announced = true;
                finished.push((job.program_id, job.name.clone(), job.stage()));
            }
        }
        for (program_id, name, stage) in finished {
            match stage {
                Stage::Done => {
                    self.library.refresh_recompiled();
                    let playing = self.game.as_ref().is_some_and(|game| game.system.title.as_ref().is_some_and(|title| title.program_id() == program_id));
                    let installed = zakuro_core::recompiled::installed(program_id);
                    if let (true, Some(game), Some(path)) = (playing, self.game.as_mut(), installed) {
                        // from here on the game runs on the new code, no reset needed
                        match zakuro_core::loader::swap_recompiled(&mut game.system, &path) {
                            Ok(()) => self.menus.message = Some(format!("{name} is recompiled, and it runs on the new code now.")),
                            Err(error) => {
                                log::warn!("could not switch {name} to its new code, {error}");
                                self.menus.message = Some(format!("{name} is recompiled. Reset it from the menu, Esc, to run it on the new code."));
                            }
                        }
                    }
                }
                Stage::Failed(error) if error != "cancelled" => {
                    self.menus.message = Some(format!("Recompiling {name} failed, {error}"));
                }
                _ => {}
            }
        }
    }

    fn act(&mut self, action: Action, event_loop: &ActiveEventLoop) {
        match action {
            Action::SaveLayout => {
                let (layers, pause, dialog, cauldron, navigation, hidden) = match self.game.as_ref().and_then(|game| game.system.aim_mouse.as_ref()) {
                    Some(mouse) => (mouse.editor_overrides(), mouse.pause_rules(), mouse.dialog_rules(), mouse.cauldron_rules(), mouse.editor_navigation(), mouse.editor_hidden_layers()),
                    None => (self.layout_overrides.clone(), self.pause_rules, self.dialog_rules, self.cauldron_rules, self.navigation.clone(), self.hidden.clone()),
                };
                let written = save_layout(&self.data_dir, &layers, pause, dialog, cauldron, navigation.clone(), hidden.clone());
                let path = layout_path(&self.data_dir);
                self.menus.message = Some(match written {
                    Ok(path) => {
                        self.layout_overrides = layers;
                        self.pause_rules = pause;
                        self.dialog_rules = dialog;
                        self.cauldron_rules = cauldron;
                        self.navigation = navigation;
                        self.hidden = hidden;
                        format!("Saved the layout to {}", path.display())
                    }
                    Err(error) => format!("Could not save {}: {error}", path.display()),
                });
            }
            Action::EditorNavigationLink(from, direction, to) => {
                if let Some(mouse) = self.game.as_mut().and_then(|game| game.system.aim_mouse.as_mut()) {
                    mouse.set_editor_navigation_link(from, direction, to);
                    self.navigation = mouse.editor_navigation();
                }
            }
            Action::EditorLayer(index, to) => {
                let overrides = self.game.as_mut().and_then(|game| game.system.aim_mouse.as_mut()).map(|mouse| {
                    mouse.set_editor_layer(index, to);
                    self.cauldron_rules = mouse.cauldron_rules();
                    mouse.editor_overrides()
                });
                if let Some(overrides) = overrides {
                    self.layout_overrides = overrides;
                }
            }
            Action::EditorLayerVisible(index, visible) => {
                if let Some(mouse) = self.game.as_mut().and_then(|game| game.system.aim_mouse.as_mut()) {
                    mouse.set_editor_layer_visible(index, visible);
                    self.cauldron_rules = mouse.cauldron_rules();
                    self.hidden = mouse.editor_hidden_layers();
                }
            }
            Action::CauldronRules(rules) => {
                self.cauldron_rules = rules;
                if let Some(mouse) = self.game.as_mut().and_then(|game| game.system.aim_mouse.as_mut()) {
                    mouse.set_cauldron_rules(rules);
                }
            }
            Action::DialogRules(rules) => {
                self.dialog_rules = rules;
                if let Some(mouse) = self.game.as_mut().and_then(|game| game.system.aim_mouse.as_mut()) {
                    mouse.set_dialog_rules(rules);
                }
            }
            Action::PauseRules(rules) => {
                self.pause_rules = rules;
                if let Some(game) = &mut self.game {
                    if let Some(mouse) = game.system.aim_mouse.as_mut() {
                        mouse.set_pause_rules(rules);
                    }
                }
            }
            Action::Play(path) => {
                if let Err(error) = self.play(&path) {
                    self.menus.message = Some(error);
                }
            }
            Action::Recompile(index) => self.recompile(index, false),
            Action::DownloadCompiler(index) => self.recompile(index, true),
            Action::Mods(program_id) => {
                if let Some(data_dir) = &self.data_dir {
                    let folder = zakuro_core::mods::dir(data_dir, program_id);
                    let made = std::fs::create_dir_all(folder.join("romfs")).and_then(|()| std::fs::create_dir_all(folder.join("textures")));
                    match made {
                        Ok(()) => show_folder(&folder),
                        Err(error) => self.menus.message = Some(format!("The mods folder can't be made: {error}")),
                    }
                }
            }
            Action::CancelRecompile(program_id) => {
                for job in self.jobs.iter().filter(|job| job.program_id == program_id) {
                    job.cancel();
                }
            }
            Action::ChooseFolder => {
                if let Some(folder) = rfd::FileDialog::new().set_title("Where your games are").pick_folder() {
                    self.settings.games = Some(folder.clone());
                    self.save_settings();
                    self.library.scan(&folder);
                }
            }
            Action::SaveState => self.save_state(),
            Action::LoadState => self.load_state(),
            Action::HostRoom => self.host_room(),
            Action::JoinRoom => {
                let address = self.settings.room_address.clone();
                self.join_room(&address);
            }
            Action::LeaveRoom => self.leave_room(),
            Action::DlpJoin(index) => self.join_download_play(index),
            Action::DlpLeave => {
                if let Some(guest) = &mut self.dlp_guest {
                    guest.leave();
                }
                self.dlp_own = None;
            }
            Action::ChooseCameraPicture => {
                let picked = rfd::FileDialog::new()
                    .set_title("A picture for the cameras")
                    .add_filter("Pictures", &["png", "jpg", "jpeg", "webp", "bmp"])
                    .pick_file();
                if let Some(file) = picked {
                    self.settings.camera = CameraSource::Picture;
                    self.settings.camera_picture = Some(file);
                    self.apply_settings();
                }
            }
            Action::ChooseBackground => {
                let picked = rfd::FileDialog::new()
                    .set_title("A picture for the library")
                    .add_filter("Pictures", &["png", "jpg", "jpeg", "webp", "bmp"])
                    .pick_file();
                if let Some(file) = picked {
                    self.settings.background = Some(file);
                    self.save_settings();
                }
            }
            Action::Rescan => {
                if let Some(folder) = self.settings.games.clone() {
                    self.library.scan(&folder);
                }
            }
            Action::Resume => {
                self.menus.menu_open = false;
                self.keyboard.release();
            }
            Action::Reset => {
                if let Some(path) = self.game.as_ref().map(|game| game.path.clone()) {
                    if let Err(error) = self.play(&path) {
                        self.menus.message = Some(error);
                    }
                }
            }
            Action::Library => self.back_to_library(),
            Action::Fullscreen => self.toggle_fullscreen(),
            Action::CopyReport => {
                if let (Some(game), Some(gui)) = (&self.game, &self.gui) {
                    gui.ctx.copy_text(report::text(&game.name, &game.system));
                    self.menus.message = Some("The game's details and the end of the log are copied. Paste them in your report and fill in what happens.".into());
                }
            }
            Action::Cheat(index, on) => {
                let Some(game) = &mut self.game else { return };
                let program_id = game.system.title.as_ref().map_or(0, |title| title.program_id());
                let Some(cheat) = game.system.cheats.get_mut(index) else { return };
                cheat.enabled = on;
                log::info!("{} {} {}", if cheat.builtin { "enhancement" } else { "cheat" }, cheat.name, if on { "on" } else { "off" });
                if cheat.builtin {
                    // an enhancement stays on for the game in the settings
                    let names = self.settings.enhancements.entry(format!("{program_id:016X}")).or_default();
                    names.retain(|name| *name != cheat.name);
                    if on {
                        names.push(cheat.name.clone());
                    }
                    self.save_settings();
                } else {
                    self.save_cheats();
                }
            }
            Action::AddCheat(name, code) => {
                let lines: Vec<String> = code.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_owned).collect();
                if lines.iter().any(|line| !zakuro_core::cheats::is_code(line)) {
                    self.menus.message = Some("Some of those lines aren't codes like 00000000 00000000, the cheat skips them.".into());
                }
                if let Some(game) = &mut self.game {
                    let name = match name.trim() {
                        "" => format!("Cheat {}", game.system.cheats.len() + 1),
                        name => name.to_owned(),
                    };
                    let mut cheat = zakuro_core::cheats::Cheat::new(&name, lines, Vec::new());
                    cheat.enabled = true;
                    game.system.cheats.push(cheat);
                }
                self.save_cheats();
            }
            Action::ReloadCheats => {
                if let (Some(path), Some(game)) = (self.cheats_path(), &mut self.game) {
                    let enhancements: Vec<_> = game.system.cheats.drain(..).filter(|cheat| cheat.builtin).collect();
                    game.system.cheats = zakuro_core::cheats::load(&path);
                    game.system.cheats.extend(enhancements);
                }
            }
            Action::CheatsFolder => {
                if let Some(folder) = self.cheats_path().and_then(|path| path.parent().map(Path::to_owned)) {
                    match std::fs::create_dir_all(&folder) {
                        Ok(()) => show_folder(&folder),
                        Err(error) => self.menus.message = Some(format!("The cheats folder can't be made: {error}")),
                    }
                }
            }
            Action::Quit => event_loop.exit(),
            Action::Settings => self.apply_settings(),
            Action::Keyboard(text, button) => {
                if let Some(game) = &mut self.game {
                    game.system.answer_keyboard(&text, button);
                }
            }
        }
    }

    fn step(&mut self, event_loop: &ActiveEventLoop) {
        if self.stop {
            event_loop.exit();
            return;
        }
        let now = Instant::now();
        if now > self.next_frame + CATCH_UP_LIMIT {
            self.next_frame = now;
        }
        self.library.poll();
        self.poll_jobs();
        self.poll_gamepads();
        self.update_prompts();
        // a room that went away takes the game's local wireless with it
        if self.room.as_ref().is_some_and(|room| !room.open()) {
            self.leave_room();
            self.menus.message = Some("The room closed, or the connection to it was lost.".into());
        }
        self.download_play();

        // a game waiting on its keyboard waits for the user, not running
        let typing = self.game.as_ref().is_some_and(|game| game.system.keyboard_request().is_some());
        let playing =
            self.game.is_some() && !self.paused && !self.menus.menu_open && !self.menus.settings_open && !typing;
        // Tab or a controller's right trigger, or the right stick's click
        // while ZR has the trigger, whichever no binding is on
        let free = |button: gilrs::Button| match self.keyboard.acting() {
            true => !self.settings.pc.bindings.binds(bindings::Source::Pad(button)),
            false => self.settings.pad.map().iter().all(|&(_, bound)| bound != button),
        };
        let trigger = [gilrs::Button::RightTrigger2, gilrs::Button::RightThumb].into_iter().find(|&button| free(button));
        let fast = playing && (self.fast_forward_key || trigger.is_some_and(|trigger| self.gamepads.holding(trigger)));
        if fast {
            // the output waits out fast forward rather than run dry
            if let Some(audio) = &self.audio {
                audio.hold();
            }
        }
        if fast != self.fast_forwarding {
            self.fast_forwarding = fast;
            if !fast {
                // back at the console's pace, with no sound piled up
                if let Some(audio) = &self.audio {
                    audio.clear();
                }
                self.next_frame = Instant::now();
                self.next_host = Instant::now();
            }
        }

        // a picture on every refresh of the display, or one a console frame
        let host_rate = playing && draws_in_between(self.high_refresh(), fast);
        if host_rate {
            self.step_at_host_rate(event_loop);
        } else {
            self.step_at_console_rate(event_loop, playing, fast);
        }
        self.report(playing);

        let now = Instant::now();
        let due = if host_rate { self.next_frame.min(self.next_host) } else { self.next_frame };
        if now < due {
            std::thread::sleep(due - now);
        }
    }

    /// the console's own pace, one picture a console frame, a few skipped
    /// while the game runs behind a presenter that waits for the display.
    fn step_at_console_rate(&mut self, event_loop: &ActiveEventLoop, playing: bool, fast: bool) {
        if playing {
            if self.mouse_down && !self.aim_captured {
                self.touch();
            }
            let start = Instant::now();
            self.emulate();
            self.spent.emulating.add(start.elapsed());
            self.follow_pause_menu();
        } else if let Some(audio) = &self.audio {
            // the sound stops with the game, which is not falling behind
            audio.hold();
        }

        self.next_frame += FRAME_TIME;
        if fast {
            // nothing to wait for
            self.next_frame = Instant::now();
        }
        // behind the schedule, showing the frame would wait on the display,
        // so it goes unshown, a few at most. a presenter that does not wait,
        // in mailbox mode, shows them all, a game that could not keep up
        // showed a fifth of its frames. fast forward shows them at the
        // console's frame rate, waiting on the display for no more
        let behind = Instant::now() > self.next_frame;
        let waits = self.backend.as_ref().is_some_and(Backend::waits_for_display);
        let unshown = if fast { Instant::now() < self.next_shown } else { skips_showing(playing, behind, waits, self.skipped) };
        if unshown {
            self.skipped += 1;
        } else {
            self.skipped = 0;
            let start = Instant::now();
            self.present(event_loop, false);
            // the next one a frame later, or right away after falling
            // behind, at the console's rate on average
            self.next_shown = (self.next_shown + FRAME_TIME).max(Instant::now() - FRAME_TIME);
            self.spent.showing.add(start.elapsed());
            self.shown.count();
        }
    }

    /// High Refresh Mode's pace: the console keeps its 59.83 Hz schedule,
    /// and the display gets a picture on every one of its own refreshes.
    fn step_at_host_rate(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if now >= self.next_frame {
            if self.mouse_down && !self.aim_captured {
                self.touch();
            }
            let start = Instant::now();
            self.emulate();
            self.spent.emulating.add(start.elapsed());
            self.follow_pause_menu();
            // the picture clock goes on across the console's frames, a
            // host refresh at a time, so an interval that is not a whole
            // number of refreshes keeps its phase instead of starting over
            // at the older frame on every frame.
            self.next_frame += FRAME_TIME;
        }
        let now = Instant::now();
        if now >= self.next_host {
            let start = Instant::now();
            self.present(event_loop, true);
            self.spent.showing.add(start.elapsed());
            self.shown.count();
            self.next_host = next_host_time(self.next_host, self.host_step, Instant::now());
        }
    }

    /// the frame rate's log and the window's title, every half second.
    fn report(&mut self, playing: bool) {
        let now = Instant::now();
        if now.saturating_duration_since(self.last_title_update) < Duration::from_millis(500) {
            return;
        }
        let shown = self.shown.read(now);
        self.last_title_update = now;
        if let Some(game) = &mut self.game {
            // read even when no frame ran, so a stall drops the game's rate
            game.fps = game.rate.read(now);
            let Spent { emulating, showing } = std::mem::take(&mut self.spent);
            log::debug!(
                target: "zakuro::fps",
                "{:.1} frames emulated and {shown:.1} shown a second, emulating took {emulating}, showing {showing}",
                game.fps
            );
        }
        if let Some(window) = &self.window {
            let title = match &self.game {
                Some(game) => format!("Zakuro - {} - {}", game.name, game.system.status_line()),
                None => "Zakuro".to_owned(),
            };
            window.set_title(&title);
        }
        let underruns = self.audio.as_ref().map_or(0, |audio| audio.take_underruns());
        if underruns > 0 && playing {
            log::warn!("the sound ran dry {underruns} times, the emulation is falling behind");
        }
    }

    /// runs one frame of the game.
    fn emulate(&mut self) {
        let top_width = self.top_width();
        // when the frame started on the host's clock, which the pictures in
        // between the last two are drawn at a fraction of
        let host_clock = self.host_time(Instant::now());
        self.older_frame_host = self.frame_host;
        self.frame_host = host_clock;
        let high_refresh = self.high_refresh();
        let Some(game) = &mut self.game else { return };
        game.system.host_clock = host_clock;
        // Kid Icarus: Uprising draws a view as wide as the top screen shows
        // and its inputs act in the context it is in, a battle or a Screen
        let mut battle = false;
        if let Some(aim_mouse) = &mut game.system.aim_mouse {
            aim_mouse.widen = top_width / 400.0;
            battle = aim_mouse.in_battle(game.system.frames);
            self.keyboard.set_context(if battle { bindings::Context::Gameplay } else { bindings::Context::Screens });
        }
        let aim = &self.settings.pc.aim;
        let sticks = match self.keyboard.acting() {
            true => gamepad::Sticks { dead_zone: aim.dead_zone, right_scale: aim.stick_sensitivity },
            false => gamepad::Sticks::default(),
        };
        let input = match &mut game.replay {
            Some(replay) => replay.input(game.frame),
            None => {
                let mut input = self.gamepads.apply(self.keyboard.state(), sticks, self.keyboard.acting());
                // Kid Icarus's gyro aims only in a battle, as its preferences say
                if self.keyboard.acting() {
                    input.gyro = self.gyro.turning(aim, battle, self.keyboard.gyro_held());
                }
                input
            }
        };
        game.frame += 1;
        if let Some(recorder) = &mut game.recorder {
            recorder.record(input);
        }
        game.system.set_input(input);
        let pictures = game.system.pictures();
        let outcome = game.system.run_frame();
        // the game's own frames are the pictures it drew, which a title
        // drawing every other console refresh has half as often. without
        // High Refresh Mode the console's frames are all the readout counts,
        // as they always have been.
        if !high_refresh || game.system.pictures() != pictures {
            game.count_frame();
        }
        let open_settings = game.system.aim_mouse.as_mut().is_some_and(|mouse| mouse.take_open_settings());
        match outcome {
            FrameOutcome::Completed => {}
            FrameOutcome::Exited => {
                log::info!("the title exited");
                self.back_to_library();
            }
            FrameOutcome::Faulted => {
                log::error!("the title stopped on a fault");
                let errors = game.system.fatal_errors.join("; ");
                if !errors.is_empty() {
                    log::error!("{errors}");
                }
                let message = format!("The game stopped on a fault. {errors}");
                self.menus.report = Some((message.clone(), report::text(&game.name, &game.system)));
                self.menus.message = Some(message);
                self.back_to_library();
            }
        }
        if open_settings {
            self.menus.settings_open = true;
            self.set_aim_captured(false);
            self.keyboard.release();
        }
    }

    /// the name the player goes by, the console's too.
    fn nickname(&self) -> String {
        match self.settings.nickname.trim() {
            "" => "Player".into(),
            name => name.chars().take(10).collect(),
        }
    }

    /// gives the running game the room the player is in, and tells the room
    /// the game.
    fn link_room(&mut self) {
        let Some(game) = &mut self.game else { return };
        game.system.room = self.room.as_ref().map(|room| room.link());
        if let Some(room) = &self.room {
            room.playing(&game.name);
        }
    }

    /// opens a room on the port chosen, and joins it.
    fn host_room(&mut self) {
        let name = match self.settings.room_name.trim() {
            "" => format!("{}'s room", self.nickname()),
            name => name.to_owned(),
        };
        let password = self.menus.room_password.clone();
        match zakuro_core::room::Server::start(self.settings.room_port, &name, &password, 16) {
            Ok(server) => {
                let address = format!("127.0.0.1:{}", server.port());
                self.room_server = Some(server);
                self.join_room(&address);
                if self.room.is_none() {
                    self.room_server = None;
                }
            }
            Err(error) => self.menus.message = Some(format!("The room could not be opened on port {}, {error}", self.settings.room_port)),
        }
    }

    /// joins the room at address.
    fn join_room(&mut self, address: &str) {
        let password = self.menus.room_password.clone();
        match zakuro_core::room::Connection::join(address.trim(), &self.nickname(), &password) {
            Ok(room) => {
                self.room = Some(room);
                self.link_room();
            }
            Err(error) => self.menus.message = Some(format!("Could not join the room, {error}.")),
        }
    }

    /// leaves the room, and closes it when it is this program's.
    fn leave_room(&mut self) {
        self.dlp_guest = None;
        self.room = None;
        self.room_server = None;
        if let Some(game) = &mut self.game {
            game.system.room = None;
        }
    }

    /// Download Play as a guest: hears the offers in the room while no game
    /// runs, and starts the child once the host's game begins.
    fn download_play(&mut self) {
        let Some(room) = self.room.as_ref().filter(|_| self.game.is_none()) else {
            self.dlp_guest = None;
            return;
        };
        let guest = self.dlp_guest.get_or_insert_with(|| zakuro_core::download_play::Guest::new(room.link()));
        guest.poll();
        match guest.end() {
            Some(zakuro_core::download_play::Phase::Begun { child, reboot }) => {
                let own = self.dlp_own.take();
                let path = match child {
                    Some(child) => self.keep_child(&child),
                    None => own.ok_or_else(|| "the game to download went missing".to_owned()),
                };
                let started = path.and_then(|path| {
                    self.wireless_reboot = Some(reboot);
                    self.play(&path)
                });
                if let Err(error) = started {
                    self.wireless_reboot = None;
                    self.menus.message = Some(format!("Download Play could not start the game, {error}."));
                }
            }
            Some(zakuro_core::download_play::Phase::Failed(why)) => {
                self.dlp_own = None;
                self.menus.message = Some(format!("Download Play stopped, {why}."));
            }
            _ => {}
        }
    }

    /// takes part in the Download Play offer at index, with the player's own
    /// copy of the child when the folder has one.
    fn join_download_play(&mut self, index: usize) {
        let node = zakuro_core::download_play::node(self.settings.console_id, &self.nickname());
        let Some(guest) = &mut self.dlp_guest else { return };
        let Some(heard) = guest.offers().get(index) else { return };
        let (host, child) = (heard.host, heard.offer.child);
        self.dlp_own = self.library.children.iter().find(|own| own.title_id == child).map(|own| own.path.clone());
        guest.join(host, node, self.dlp_own.is_some());
    }

    /// keeps a child the host sent, for it to run from.
    fn keep_child(&self, child: &[u8]) -> Result<PathBuf, String> {
        let cia = zakuro_fs::Cia::read(child).map_err(|_| "the host sent something that isn't a game".to_owned())?;
        let folder = self.data_dir.clone().unwrap_or_else(std::env::temp_dir).join("dlp");
        let path = folder.join(format!("{:016X}.cia", cia.title_id));
        std::fs::create_dir_all(&folder).and_then(|()| std::fs::write(&path, child)).map_err(|error| error.to_string())?;
        Ok(path)
    }

    /// the file of a slot of the running game's saved states.
    fn state_path(&self, slot: u8) -> Option<PathBuf> {
        let program_id = self.game.as_ref()?.system.title.as_ref()?.program_id();
        Some(self.data_dir.as_ref()?.join("states").join(format!("{program_id:016X}")).join(format!("{slot}.state")))
    }

    /// saves the running game's state to the slot chosen.
    fn save_state(&mut self) {
        let slot = self.menus.slot;
        let Some(path) = self.state_path(slot) else { return };
        let Some(game) = &mut self.game else { return };
        let started = Instant::now();
        let saved = game.system.save_state().and_then(|bytes| {
            let parent = path.parent().expect("a state's file is in a folder");
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            // written beside it first, a state cut short never takes the
            // place of a whole one
            let partial = path.with_extension("state.new");
            std::fs::write(&partial, &bytes).and_then(|()| std::fs::rename(&partial, &path)).map_err(|error| error.to_string())
        });
        match saved {
            Ok(()) => {
                log::info!("saved the state to {} in {:?}", path.display(), started.elapsed());
                self.menus.toast = Some((format!("Saved to slot {slot}"), Instant::now()));
            }
            Err(error) => self.menus.message = Some(format!("The state could not be saved, {error}")),
        }
        // the time it took is not the game's to make up
        self.next_frame = Instant::now();
    }

    /// loads the running game's state from the slot chosen.
    fn load_state(&mut self) {
        let slot = self.menus.slot;
        let Some(path) = self.state_path(slot) else { return };
        let Some(game) = &mut self.game else { return };
        let loaded = std::fs::read(&path)
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => format!("Slot {slot} has no state saved."),
                _ => format!("The state could not be read, {error}"),
            })
            .and_then(|bytes| game.system.load_state(&bytes).map_err(|error| format!("The state could not be loaded, {error}.")));
        match loaded {
            Ok(()) => {
                log::info!("loaded the state from {}", path.display());
                if self.options.menu_editor {
                    if let Some(mouse) = self.game.as_mut().and_then(|game| game.system.aim_mouse.as_mut()) {
                        mouse.set_editor_navigation(self.navigation.clone());
                        mouse.set_editor_hidden_layers(self.hidden.clone());
                    }
                }
                self.menus.toast = Some((format!("Loaded slot {slot}"), Instant::now()));
                self.menus.menu_open = false;
                if let Some(audio) = &self.audio {
                    audio.clear();
                }
            }
            Err(error) => self.menus.message = Some(error),
        }
        self.next_frame = Instant::now();
    }

    /// the file of the running game's cheats.
    fn cheats_path(&self) -> Option<PathBuf> {
        let program_id = self.game.as_ref()?.system.title.as_ref()?.program_id();
        Some(zakuro_core::cheats::path(self.data_dir.as_ref()?, program_id))
    }

    /// writes the running game's cheats to their file.
    fn save_cheats(&mut self) {
        let (Some(path), Some(game)) = (self.cheats_path(), &self.game) else { return };
        if let Err(error) = zakuro_core::cheats::save(&path, &game.system.cheats) {
            self.menus.message = Some(format!("The cheats can't be saved: {error}"));
        }
    }

    /// the menus, and the actions they asked for, done.
    fn interface(&mut self, event_loop: &ActiveEventLoop) -> Overlay {
        // when the slot chosen was saved to, while the menu shows it
        let saved = self
            .menus
            .menu_open
            .then(|| self.state_path(self.menus.slot))
            .flatten()
            .and_then(|path| std::fs::metadata(path).ok()?.modified().ok())
            .map(ago);
        // the host's pictures read apart from the game's own frames, so a
        // dropped picture and a dropped frame can be told apart
        let high_refresh = self.high_refresh();
        let shown = self.shown.rate();
        let canvas = {
            let layout_kind = self.layout;
            let top_width = self.top_width();
            let integer = self.settings.integer_scale;
            self.window.as_ref().and_then(|window| {
                let size = window.inner_size();
                let (top, _) = layout(size.width, size.height, layout_kind.screens(), top_width, integer);
                top.map(|top| [top.x, top.y, top.width, top.height])
            })
        };
        let (Some(gui), Some(window)) = (&mut self.gui, &self.window) else { return Overlay::default() };
        let show_fps = self.settings.show_fps;
        let editor_open = self.menus.editor_open;
        let editor_enabled = self.options.menu_editor;
        let (editor_layers, editor_visibility, navigation_controls, navigation_directions, editor_widen) = self.game.as_ref().filter(|_| editor_open).and_then(|game| game.system.aim_mouse.as_ref())
            .map_or((Vec::new(), Vec::new(), Vec::new(), [true; 4], 1.0), |mouse| (mouse.editor_layers(), mouse.editor_layer_visibility(), mouse.editor_navigation_controls(), mouse.editor_navigation_directions(), mouse.widen()));
        let game = self.game.as_ref().map(|game| {
            let fps = menus::FpsReadout::new(high_refresh, shown, game.fps);
            // the window of the Screen the editor edits: the Cauldron, the
            // Retreat Dialog, or Pause's rules for anything else
            let rules = game.system.aim_mouse.as_ref().map(|mouse| {
                if let Some(cauldron) = mouse.editor_cauldron_rules() {
                    menus::EditorRules::Cauldron(cauldron)
                } else if let Some(dialog) = mouse.editor_dialog_rules() {
                    menus::EditorRules::RetreatDialog(dialog)
                } else {
                    menus::EditorRules::Pause(mouse.pause_rules())
                }
            });
            (game.name.clone(), fps, game.system.recompiled.is_some(), rules)
        });
        let fast = self.fast_forwarding;
        let keyboard = self.game.as_ref().and_then(|game| game.system.keyboard_request().cloned());
        let room = self.room.as_ref().filter(|_| self.menus.multiplayer_open).map(|connection| {
            let members = connection.members();
            menus::RoomView {
                name: connection.room().to_owned(),
                download_play: self.dlp_guest.as_ref().map(|guest| download_play_view(guest, &members, &self.library)),
                members: members.into_iter().map(|member| (member.nickname, member.game)).collect(),
                port: self.room_server.as_ref().map(|server| server.port()),
            }
        });
        let cheats = self.game.as_ref().filter(|_| self.menus.cheats_open).map(|game| {
            game.system.cheats.iter().map(|cheat| (cheat.name.clone(), cheat.enabled, cheat.notes.join("\n"), cheat.builtin)).collect::<Vec<_>>()
        });
        let (menus, library, settings, jobs) = (&mut self.menus, &self.library, &mut self.settings, &self.jobs);
        let mut actions = Vec::new();
        let overlay = gui.frame(window, |ui| {
            match &game {
                Some((name, fps, recompiled, rules)) => actions.extend(menus.game(
                    ui, name, show_fps.then_some(*fps), *recompiled, fast, jobs, saved.as_deref(), *rules,
                    editor_layers.clone(), editor_visibility.clone(), navigation_controls.clone(), navigation_directions, editor_widen, canvas, editor_open, editor_enabled,
                )),
                None => actions.extend(menus.library(ui, library, settings, jobs)),
            }
            actions.extend(menus.settings(ui.ctx(), settings));
            if menus.multiplayer_open {
                actions.extend(menus.multiplayer(ui.ctx(), settings, room.as_ref()));
            }
            if let Some(cheats) = &cheats {
                actions.extend(menus.cheats(ui.ctx(), cheats));
            }
            if let Some(request) = &keyboard {
                actions.extend(menus.keyboard(ui.ctx(), request));
            }
            actions.extend(menus.compiler_offer(ui.ctx()));
            menus.message(ui.ctx());
        });
        self.library.changed = false;
        for action in actions {
            self.act(action, event_loop);
        }
        overlay
    }

    fn present(&mut self, event_loop: &ActiveEventLoop, host_rate: bool) {
        let overlay = self.interface(event_loop);
        let blank = |screen: Screen| ((std::sync::Arc::new(Vec::new()), screen.width(), screen.height()), None);
        // the right eye's picture only while the 3D shows
        let stereo = self.settings.stereo != settings::Stereo3d::Off;
        // the title's own at a frame's time, or one drawn in between the
        // last two, on every refresh of the display; fast forward shows the
        // title's own only
        let high_refresh = self.high_refresh();
        let in_between = host_rate;
        let real_only = high_refresh && self.fast_forwarding;
        let host_time = self.shown_host;
        let (top, bottom, right) = match &mut self.game {
            Some(game) if in_between => (
                shown_at_host_time(&mut game.system, Screen::Top, host_time),
                shown_at_host_time(&mut game.system, Screen::Bottom, host_time),
                stereo.then(|| shown_right_at_host_time(&mut game.system, host_time)).flatten(),
            ),
            Some(game) if real_only => (
                shown_real(&mut game.system, Screen::Top),
                shown_real(&mut game.system, Screen::Bottom),
                stereo.then(|| shown_right_real(&mut game.system)).flatten(),
            ),
            Some(game) => (
                shown(&mut game.system, Screen::Top),
                shown(&mut game.system, Screen::Bottom),
                stereo.then(|| shown_right(&mut game.system)).flatten(),
            ),
            None => (blank(Screen::Top), blank(Screen::Bottom), None),
        };
        if in_between {
            // the next refresh a display's interval further through the
            // last two frames
            self.shown_host += self.host_step;
        }
        let right = right.unwrap_or_else(|| blank(Screen::Top));
        let Some(backend) = &mut self.backend else { return };
        let (((top, top_width, top_height), top_gpu), ((bottom, bottom_width, bottom_height), bottom_gpu)) = (top, bottom);
        let ((right, right_width, right_height), right_gpu) = right;
        let result = backend.present(
            ScreenImage { width: top_width, height: top_height, pixels: &top, gpu: top_gpu },
            ScreenImage { width: bottom_width, height: bottom_height, pixels: &bottom, gpu: bottom_gpu },
            ScreenImage { width: right_width, height: right_height, pixels: &right, gpu: right_gpu },
            &overlay,
        );
        match result {
            Ok(()) | Err(PresentError::OutOfDate) => {}
            Err(error) => {
                log::error!("presentation failed: {error}");
                event_loop.exit();
            }
        }
    }
}

/// what the multiplayer window shows of Download Play: the offers in the
/// room, and where the guest is.
fn download_play_view(guest: &zakuro_core::download_play::Guest, members: &[zakuro_core::room::Member], library: &Library) -> menus::DlpView {
    use zakuro_core::download_play::Phase;
    let offers = guest
        .offers()
        .iter()
        .map(|heard| {
            let offer = &heard.offer;
            let smdh = library::Smdh::parse(&offer.smdh);
            let own = library.children.iter().any(|own| own.title_id == offer.child);
            let blocked = if offer.guests >= offer.most {
                Some("The host's game takes no more guests.".to_owned())
            } else if !offer.sends_child && !own {
                Some(
                    "The game to download is encrypted on the card, so either player needs a decrypted copy of it: \
                     decrypt the .cia in the card's Download Play partition with GodMode9 and put it in the games folder."
                        .to_owned(),
                )
            } else {
                None
            };
            menus::OfferView {
                child: offer.child,
                title: smdh.as_ref().map(|smdh| smdh.name.clone()).filter(|name| !name.is_empty()).unwrap_or_else(|| format!("{:016X}", offer.child)),
                host: members.iter().find(|member| member.mac == heard.host).map(|member| member.nickname.clone()).unwrap_or_default(),
                icon: smdh.map(|smdh| smdh.icon),
                guests: offer.guests,
                most: offer.most,
                blocked,
            }
        })
        .collect();
    let doing = guest.phase().map(|phase| match phase {
        Phase::Asking => ("Asking the host to join.".to_owned(), None),
        Phase::Waiting => ("Joined. The game downloads once the host starts it.".to_owned(), None),
        Phase::Receiving { got, total } => ("Downloading the game.".to_owned(), Some(*got as f32 / (*total).max(1) as f32)),
        Phase::Ready => ("Ready. Waiting for the host to start the game.".to_owned(), None),
        Phase::Begun { .. } => ("Starting the game.".to_owned(), None),
        Phase::Failed(why) => (why.clone(), None),
    });
    menus::DlpView { offers, doing }
}

/// a screen's picture at a host time, straight from the GPU when it shares
/// the renderer's device: the title's own at a frame's time, or one drawn in
/// between the last two at the fraction of the way from the older.
fn shown_at_host_time(system: &mut System, screen: Screen, host_time: Duration) -> (zakuro_core::Screen, Option<GpuScreen>) {
    let (gpu, _) = system.gpu_picture_at_host_time(screen, false, host_time);
    if let Some(gpu) = gpu {
        let scale = system.gpu.scale();
        return ((std::sync::Arc::new(Vec::new()), screen.width() * scale, screen.height() * scale), Some(gpu));
    }
    (system.read_screen_scaled(screen), None)
}

/// the top screen's picture at a host time for the right eye, when the
/// title draws in 3D.
fn shown_right_at_host_time(system: &mut System, host_time: Duration) -> Option<(zakuro_core::Screen, Option<GpuScreen>)> {
    let (gpu, _) = system.gpu_picture_at_host_time(Screen::Top, true, host_time);
    if let Some(gpu) = gpu {
        let scale = system.gpu.scale();
        return Some(((std::sync::Arc::new(Vec::new()), Screen::Top.width() * scale, Screen::Top.height() * scale), Some(gpu)));
    }
    system.read_right_eye_scaled().map(|screen| (screen, None))
}

/// a screen's own newest picture with none drawn in between, for fast
/// forward, which shows the title's frames only.
fn shown_real(system: &mut System, screen: Screen) -> (zakuro_core::Screen, Option<GpuScreen>) {
    if let Some(gpu) = system.gpu_screen_real(screen) {
        let scale = system.gpu.scale();
        return ((std::sync::Arc::new(Vec::new()), screen.width() * scale, screen.height() * scale), Some(gpu));
    }
    (system.read_screen_scaled(screen), None)
}

/// the top screen's own newest picture for the right eye, with none drawn
/// in between.
fn shown_right_real(system: &mut System) -> Option<(zakuro_core::Screen, Option<GpuScreen>)> {
    if let Some(gpu) = system.gpu_right_eye_real() {
        let scale = system.gpu.scale();
        return Some(((std::sync::Arc::new(Vec::new()), Screen::Top.width() * scale, Screen::Top.height() * scale), Some(gpu)));
    }
    system.read_right_eye_scaled().map(|screen| (screen, None))
}

/// a screen as the presenter takes it, straight from the GPU when it shares
/// the renderer's device and the GPU drew what the screen shows, else its
/// pixels.
fn shown(system: &mut System, screen: Screen) -> (zakuro_core::Screen, Option<GpuScreen>) {
    if let Some(gpu) = system.gpu_screen(screen) {
        let scale = system.gpu.scale();
        return ((std::sync::Arc::new(Vec::new()), screen.width() * scale, screen.height() * scale), Some(gpu));
    }
    (system.read_screen_scaled(screen), None)
}

/// the top screen's picture for the right eye, as shown gives the screens,
/// when the game draws in 3D.
fn shown_right(system: &mut System) -> Option<(zakuro_core::Screen, Option<GpuScreen>)> {
    if let Some(gpu) = system.gpu_right_eye() {
        let scale = system.gpu.scale();
        return Some(((std::sync::Arc::new(Vec::new()), Screen::Top.width() * scale, Screen::Top.height() * scale), Some(gpu)));
    }
    system.read_right_eye_scaled().map(|screen| (screen, None))
}

/// keeps the renderer kind referenced even when a backend feature is off.
const _: RendererKind = RendererKind::Software;

#[cfg(test)]
mod tests {
    use super::*;
    use zakuro_core::aim_mouse::{NavigationDirection, NavigationLink, RuleScale};

    /// a fresh folder for one test's `menu-layout.toml`.
    fn temp_data_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("zakuro-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder for the test's layout");
        dir
    }

    #[test]
    fn saved_layout_round_trips_pause_rules_layers_navigation_and_hidden() {
        let dir = temp_data_dir("layout-round-trip");
        let mut rules = PauseRules::default();
        rules.center.to[0] = 60.0;
        rules.left.scale = RuleScale::Fixed;
        rules.corner.anchor = 0.25;
        let layers = vec![(13, 0, [7.0, 8.0, 90.0, 40.0]), (5, 2, [0.0, 1.0, 30.0, 20.0])];
        let navigation = vec![
            NavigationLink { screen: 13, from: 0, direction: NavigationDirection::Up, to: 2 },
            NavigationLink { screen: 13, from: 2, direction: NavigationDirection::Left, to: 2 },
            NavigationLink { screen: 5, from: 1, direction: NavigationDirection::Down, to: 0 },
        ];
        let hidden = vec![(13, 5), (5, 2)];
        let mut cauldron = CauldronRules::default();
        cauldron.rule_mut(zakuro_core::aim_mouse::CauldronRole::Back).visible = false;
        cauldron.rule_mut(zakuro_core::aim_mouse::CauldronRole::Top).to = [20.0, 30.0, 150.0, 100.0];
        let mut dialog = DialogRules::default();
        dialog.panel.to = [200.0, 20.0, 160.0, 120.0];
        dialog.span.visible = false;
        let path = save_layout(&Some(dir.clone()), &layers, rules, dialog, cauldron, navigation.clone(), hidden.clone()).expect("the layout is written");
        assert_eq!(path, layout_path(&Some(dir.clone())));
        let loaded = load_layout(&Some(dir.clone())).expect("the layout is read");
        let loaded_layers: Vec<_> = loaded.layers.into_iter().map(|layer| (layer.screen, layer.part, layer.to)).collect();
        assert_eq!(loaded_layers, layers);
        assert_eq!(loaded.pause, Some(rules));
        assert_eq!(loaded.cauldron, Some(cauldron));
        assert_eq!(loaded.dialog, Some(dialog));
        assert_eq!(loaded.navigation, navigation);
        assert_eq!(loaded.hidden, hidden);
        std::fs::remove_dir_all(&dir).expect("the test's folder goes");
    }

    /// a layout saved before the Cauldron's ring had a role keeps its
    /// Cauldron rules, and the ring gets its default place.
    #[test]
    fn a_cauldron_layout_saved_without_the_ring_still_loads() {
        use zakuro_core::aim_mouse::CauldronRole;
        let dir = temp_data_dir("layout-before-ring");
        let mut cauldron = CauldronRules::default();
        cauldron.rule_mut(CauldronRole::Back).visible = false;
        let path = save_layout(&Some(dir.clone()), &[], PauseRules::default(), DialogRules::default(), cauldron, Vec::new(), Vec::new()).expect("the layout is written");
        let mut file: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        file["cauldron"].as_array_mut().expect("the Cauldron's rules are a list").pop();
        std::fs::write(&path, toml::to_string(&file).unwrap()).unwrap();
        let loaded = load_layout(&Some(dir.clone())).expect("the old layout is read").cauldron.expect("its Cauldron rules are kept");
        assert!(!loaded.rule(CauldronRole::Back).visible);
        assert_eq!(loaded.rule(CauldronRole::Ring), CauldronRules::default().rule(CauldronRole::Ring));
        std::fs::remove_dir_all(&dir).expect("the test's folder goes");
    }

    /// a layout saved before the Retreat Dialog had rules of its own keeps
    /// everything else, and the dialog loads with none (the defaults).
    #[test]
    fn a_layout_saved_without_retreat_dialog_rules_still_loads() {
        let dir = temp_data_dir("layout-before-dialog");
        let mut pause = PauseRules::default();
        pause.center.to[0] = 60.0;
        let path = save_layout(&Some(dir.clone()), &[], pause, DialogRules::default(), CauldronRules::default(), Vec::new(), Vec::new()).expect("the layout is written");
        let mut file: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        file.remove("dialog").expect("the dialog's rules are written");
        std::fs::write(&path, toml::to_string(&file).unwrap()).unwrap();
        let loaded = load_layout(&Some(dir.clone())).expect("the old layout is read");
        assert_eq!(loaded.pause, Some(pause));
        assert_eq!(loaded.dialog, None);
        std::fs::remove_dir_all(&dir).expect("the test's folder goes");
    }

    /// a Scripted layer hidden without ever being moved is saved on its own.
    #[test]
    fn a_hidden_layer_is_saved_without_a_move() {
        let dir = temp_data_dir("layout-hidden");
        let hidden = vec![(11, 5), (4, 1)];
        save_layout(&Some(dir.clone()), &[], PauseRules::default(), DialogRules::default(), CauldronRules::default(), Vec::new(), hidden.clone())
            .expect("the layout with only a hidden layer is written");
        let loaded = load_layout(&Some(dir.clone())).expect("the layout is read");
        assert!(loaded.layers.is_empty(), "hiding a layer wrote a move");
        assert_eq!(loaded.hidden, hidden, "the hidden layer did not come back");
        std::fs::remove_dir_all(&dir).expect("the test's folder goes");
    }

    /// a file written before Pause rules were kept still loads its layers.
    #[test]
    fn a_layout_without_pause_rules_still_loads() {
        let dir = temp_data_dir("layout-legacy");
        std::fs::write(
            layout_path(&Some(dir.clone())),
            "[[layers]]\nscreen = 13\npart = 0\nto = [1.0, 2.0, 3.0, 4.0]\n",
        )
        .expect("the old layout is written");
        let loaded = load_layout(&Some(dir.clone())).expect("the old layout is read");
        let loaded_layers: Vec<_> = loaded.layers.into_iter().map(|layer| (layer.screen, layer.part, layer.to)).collect();
        assert_eq!(loaded_layers, vec![(13, 0, [1.0, 2.0, 3.0, 4.0])]);
        assert_eq!(loaded.pause, None);
        assert!(loaded.navigation.is_empty(), "a layout without links has none");
        assert!(loaded.hidden.is_empty(), "a layout without hidden layers hides none");
        std::fs::remove_dir_all(&dir).expect("the test's folder goes");
    }

    /// Pause rules written before visibility existed load every rule shown.
    #[test]
    fn old_pause_rules_without_visibility_stay_visible() {
        let dir = temp_data_dir("layout-legacy-pause");
        let rule = "from = [0.0, 0.0, 320.0, 240.0]\nto = [0.0, 0.0, 400.0, 240.0]\nanchor = 0.5\nscale = \"Narrow\"\n";
        let mut text = String::new();
        for name in ["span", "center", "left", "right", "corner"] {
            text.push_str(&format!("[pause.{name}]\n{rule}\n"));
        }
        std::fs::write(layout_path(&Some(dir.clone())), text).expect("the old layout is written");
        let loaded = load_layout(&Some(dir.clone())).expect("the old layout is read");
        let rules = loaded.pause.expect("the old Pause rules");
        assert!([rules.span, rules.center, rules.left, rules.right, rules.corner].iter().all(|rule| rule.visible), "old rules were hidden");
        std::fs::remove_dir_all(&dir).expect("the test's folder goes");
    }

    /// a damaged layout is reported, not silently treated as the default.
    #[test]
    fn a_malformed_layout_is_reported() {
        let dir = temp_data_dir("layout-malformed");
        std::fs::write(layout_path(&Some(dir.clone())), "layers = [").expect("the damaged layout is written");
        assert!(load_layout(&Some(dir.clone())).is_err());
        std::fs::remove_dir_all(&dir).expect("the test's folder goes");
    }

    /// a save that cannot be written says so instead of claiming success.
    #[test]
    fn a_layout_that_cannot_be_written_reports_an_error() {
        let dir = temp_data_dir("layout-error");
        let blocked = dir.join("not-a-folder");
        std::fs::write(&blocked, b"").expect("a file stands where the folder would go");
        assert!(save_layout(&Some(blocked), &[], PauseRules::default(), DialogRules::default(), CauldronRules::default(), Vec::new(), Vec::new()).is_err());
        std::fs::remove_dir_all(&dir).expect("the test's folder goes");
    }

    #[test]
    fn wheel_scrolls_vertically_and_shift_scrolls_horizontally() {
        assert_eq!(menu_wheel_delta(MouseScrollDelta::LineDelta(0.0, -1.0), false), [0.0, 1.0]);
        assert_eq!(menu_wheel_delta(MouseScrollDelta::LineDelta(0.0, -1.0), true), [1.0, 0.0]);
        assert_eq!(menu_wheel_delta(MouseScrollDelta::LineDelta(1.0, 0.0), false), [-1.0, 0.0]);
        assert_eq!(menu_wheel_delta(MouseScrollDelta::PixelDelta((0.0, 10.0).into()), false), [0.0, -0.25]);
    }

    /// a picture on the refresh after the one scheduled, or a refresh on
    /// from now when it is late, so missed refreshes never pile up.
    #[test]
    fn a_late_picture_does_not_show_a_burst_of_them() {
        let now = Instant::now();
        let step = Duration::from_millis(8);
        assert_eq!(next_host_time(now, step, now), now + step);
        assert_eq!(next_host_time(now - Duration::from_secs(1), step, now), now + step);
    }

    /// fast forward shows the title's own frames only, never a picture
    /// drawn in between, whether the mode was on or off.
    #[test]
    fn fast_forward_shows_real_frames_only() {
        assert!(draws_in_between(true, false));
        assert!(!draws_in_between(true, true), "fast forward skips the pictures in between");
        assert!(!draws_in_between(false, false));
        assert!(!draws_in_between(false, true));
    }

    /// a frame goes unshown only while the game runs behind with a
    /// presenter that waits for the display, a few in a row at most.
    #[test]
    fn frames_go_unshown_only_when_showing_would_wait() {
        assert!(skips_showing(true, true, true, 0));
        assert!(skips_showing(true, true, true, MAX_SKIPPED - 1));
        assert!(!skips_showing(true, true, true, MAX_SKIPPED), "a few in a row at most");
        assert!(!skips_showing(true, true, false, 0), "mailbox shows every frame");
        assert!(!skips_showing(true, false, true, 0), "on time");
        assert!(!skips_showing(false, true, true, 0), "paused");
    }

    /// without High Refresh Mode the readout is the game's own rate.
    #[test]
    fn the_frame_rate_is_the_game_rate_with_high_refresh_off() {
        assert_eq!(menus::FpsReadout::new(false, 144.0, 60.0).text(), "60 fps");
    }

    /// with High Refresh Mode on the readout keeps the shown pictures and
    /// the game's frames apart.
    #[test]
    fn the_frame_rate_tells_shown_pictures_and_game_frames_apart() {
        assert_eq!(menus::FpsReadout::new(true, 144.0, 60.0).text(), "144 shown / 60 game");
    }

    /// a stall runs no game frame while the display keeps its pictures
    /// coming, so the game's number drops and the shown one stays.
    #[test]
    fn a_stall_drops_the_game_rate_and_keeps_the_shown_rate() {
        let start = Instant::now();
        let mut shown = FrameRate { counted: 0, since: start, rate: 0.0 };
        let mut game = FrameRate { counted: 0, since: start, rate: 0.0 };
        for _ in 0..144 {
            shown.count();
        }
        for _ in 0..60 {
            game.count();
        }
        assert_eq!(shown.read(start + Duration::from_secs(1)), 144.0);
        assert_eq!(game.read(start + Duration::from_secs(1)), 60.0);

        // the game stalls: a second goes by with no frame, the pictures
        // carry on
        for _ in 0..144 {
            shown.count();
        }
        let second = start + Duration::from_secs(2);
        assert_eq!(shown.read(second), 144.0);
        assert_eq!(game.read(second), 0.0);
        let readout = menus::FpsReadout::new(true, shown.rate(), game.rate());
        assert_eq!(readout.text(), "144 shown / 0 game");
    }
}
