//! the emulated console, CPU, memory, HLE kernel, HLE services and GPU, and
//! the loop that drives them.

pub mod aim_mouse;
pub mod camera;
pub mod cheats;
pub mod enhancements;
pub mod cro;
pub mod download_play;
pub mod hints;
pub mod idle;
pub mod kernel;
pub mod loader;
pub mod memory;
pub mod mods;
pub mod replay;
pub mod room;
pub mod recompiled;
pub mod services;
pub mod state;

use std::collections::{BTreeMap, BTreeSet};

use zakuro_common::memory_map::*;
use zakuro_common::ConsoleModel;
use zakuro_cpu::{Cpu, Exit};
use zakuro_fs::Title;
use zakuro_gpu::{Gpu, GpuMemory, Renderer, SoftwareRenderer};

use kernel::thread::{ThreadId, WaitResult, WaitSyscall, THREAD_EXIT_MAGIC};
use kernel::Kernel;
use memory::{Memory, MemoryState, Permission};
use services::hid::InputState;
use services::ServiceState;

/// console settings the frontend can change.
#[derive(Debug, Clone)]
pub struct Config {
    pub model: ConsoleModel,
    pub new3ds: bool,
    /// 0 = Japan, 1 = USA, 2 = Europe.
    pub region: u8,
    pub language: u8,
    pub slider_3d: f32,
    /// a library 3dsrecomp built for the title, or a directory holding one
    /// named after its title id.
    pub recompiled: Option<std::path::PathBuf>,
    /// recompiled code linked into the program, which comes before a
    /// library.
    pub linked: Option<recompiled::Linked>,
    /// with no library given, run the one 3dsrecomp build installed for the
    /// title, if it did.
    pub find_recompiled: bool,
    /// where saves and dumped system files live, the working directory
    /// when there is none.
    pub data_dir: Option<std::path::PathBuf>,
    /// draw on the host's GPU through Vulkan, when there is one that can.
    pub hardware_renderer: bool,
    /// draw the pictures of the texture pack in the title's mods folder in
    /// place of its textures, which takes the host's GPU.
    pub texture_packs: bool,
    /// the title's update, an update's CIA, whose code and data it runs.
    pub update: Option<std::path::PathBuf>,
    /// the title's downloadable content, DLC CIAs.
    pub dlc: Vec<std::path::PathBuf>,
    /// the enhancements turned on, by name, for each title by its id.
    pub enhancements: std::collections::BTreeMap<u64, Vec<String>>,
    /// how many times the console's resolution the host's GPU draws at.
    pub resolution: u32,
    /// the Vulkan presenter's device, to draw on and show the screens
    /// straight from, when it can.
    pub device: Option<std::sync::Arc<zakuro_gpu::SharedDevice>>,
    /// the time the console's clock starts at, in milliseconds since 1900,
    /// the host's when none, a fixed one makes runs repeat exactly.
    pub clock: Option<u64>,
    /// High Refresh Mode, a picture on every refresh of the host drawn in
    /// between the title's last two, see System::pace, when the host's GPU
    /// draws the 3D and shows the screens straight from its device.
    pub interpolate: bool,
    /// the console owner's name, up to ten characters, which titles show
    /// and local wireless tells others.
    pub username: String,
    /// the number that tells this console apart from others over local
    /// wireless, the configuration's block 0x00090000.
    pub console_id: u64,
    /// the game's Download Play child decrypted, which a host sends its
    /// guests. the one on a card is encrypted.
    pub dlp_child: Option<std::path::PathBuf>,
    /// what a Download Play child runs with, the host's address and the
    /// passphrase of its network, APT:GetWirelessRebootInfo's answer.
    pub wireless_reboot: Option<[u8; 16]>,
    /// the interpreter skips to the next event past loops that change
    /// nothing, see idle.
    pub skip_idle_loops: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            model: ConsoleModel::Old3ds,
            new3ds: false,
            region: services::cfg::REGION_USA,
            language: services::cfg::LANGUAGE_ENGLISH,
            slider_3d: 0.0,
            recompiled: None,
            linked: None,
            find_recompiled: false,
            data_dir: None,
            hardware_renderer: false,
            texture_packs: true,
            update: None,
            dlc: Vec::new(),
            enhancements: std::collections::BTreeMap::new(),
            resolution: 1,
            device: None,
            clock: None,
            interpolate: false,
            username: "Zakuro".into(),
            console_id: 0x5A4B_0000_0000_0001,
            dlp_child: None,
            wireless_reboot: None,
            skip_idle_loops: true,
        }
    }
}

/// how a frame of emulation ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameOutcome {
    /// a full frame's worth of cycles ran.
    Completed,
    /// the title called svcExitProcess.
    Exited,
    /// the title called svcBreak, or hit an instruction we do not have.
    Faulted,
}

/// a screen's picture as RGBA, and its width and height.
pub type Screen = (std::sync::Arc<Vec<u8>>, u32, u32);

/// what a picture shown at a host time is: the title's own frame, or one
/// blended in between the last two at a fraction of the way from the older
/// to the newer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PictureKind {
    /// the title's own picture, shown as it is.
    Real,
    /// the picture in between the older frame's and the newer's, t of the
    /// way from the older to the newer, 0 for the older and 1 for the newer.
    InBetween(f32),
}

impl PictureKind {
    /// whether the picture is the title's own.
    pub fn is_real(&self) -> bool {
        matches!(self, PictureKind::Real)
    }

    /// how far from the older frame's picture to the newer's this one is,
    /// none for the title's own.
    pub fn t(&self) -> Option<f32> {
        match self {
            PictureKind::Real => None,
            PictureKind::InBetween(t) => Some(*t),
        }
    }
}

/// the picture a screen shows at a host time, and what it is.
#[derive(Debug, Clone)]
pub struct HostPicture {
    /// the picture, as read_screen_scaled gives the screen's.
    pub screen: Screen,
    /// the title's own at a frame's time, or one in between at another.
    pub kind: PictureKind,
}

/// the top screen's pictures for both eyes at a host time, and what they
/// are. the kind is decided once for the pair, so both eyes are the title's
/// own or the same picture in between at the same t, and the 3D image never
/// tears between eyes.
#[derive(Debug, Clone)]
pub struct StereoPicture {
    /// the picture the left eye shows.
    pub left: HostPicture,
    /// the picture the right eye shows, when the title draws in 3D.
    pub right: Option<HostPicture>,
}

/// the top screen's pictures for both eyes on the host's GPU at a host
/// time, and what they are, for a presenter sharing the device. the kind is
/// the pair's, as StereoPicture has it, so neither eye is shown at a t the
/// other is not.
#[derive(Debug, Clone, Copy)]
pub struct GpuStereoPicture {
    /// the picture the left eye shows on the GPU, as gpu_screen gives it.
    pub left: Option<zakuro_gpu::GpuScreen>,
    /// the picture the right eye shows, when the title draws in 3D.
    pub right: Option<zakuro_gpu::GpuScreen>,
    /// the title's own at a frame's time, or one in between at another.
    pub kind: PictureKind,
}

/// frame interpolation's count of the refreshes, see System::pace.
#[derive(Default)]
struct Pacing {
    /// high refresh mode is on, from set_interpolation. the pacer decides
    /// the pictures only while it is, and one decision is the pair of eyes'.
    on: bool,
    refreshes: u64,
    /// the refresh the top screen got its last new picture at, and that
    /// picture.
    new_at: u64,
    last: Option<zakuro_gpu::ScreenRef>,
    /// how the draws of the frame before it matched the frame before that.
    matches: Option<zakuro_gpu::Matches>,
    /// the screens show the pictures in between at this refresh, and the
    /// title's own at the next.
    between: bool,
    /// the refreshes that showed pictures in between so far.
    shown: u64,
    /// the new pictures the top screen has shown, for the frontend's readout
    /// of the game's own frames. a title drawing every other refresh has one
    /// every other console refresh.
    pictures: u64,
    /// the host time the last new picture arrived at, when pacing knows it.
    newer_host_time: Option<std::time::Duration>,
    /// the host time the picture before that arrived at, when pacing knows it.
    older_host_time: Option<std::time::Duration>,
}

impl Pacing {
    /// the kind of picture the title's frames show at a host time, without
    /// the picture. the title's own at a frame's time, or one in between
    /// when it draws every other refresh and the host time falls between its
    /// frames. the older frame's at the older time, t=0, and the newest at
    /// its own, t=1. a title that stalled, more than two refreshes without
    /// a new picture, shows its own. it is the pictures' kind and not an
    /// eye's: both eyes of a 3D picture come from the same two frames, so
    /// both are this one, at this one t.
    fn kind_at_host_time(&self, host_time: std::time::Duration) -> PictureKind {
        if self.refreshes - self.new_at > 2 {
            return PictureKind::Real;
        }
        let (Some(older), Some(newer)) = (self.older_host_time, self.newer_host_time) else {
            return PictureKind::Real;
        };
        if newer <= older {
            return PictureKind::Real;
        }
        let total = newer.as_secs_f32() - older.as_secs_f32();
        if total <= 0.0 {
            return PictureKind::Real;
        }
        let elapsed = host_time.saturating_sub(older).as_secs_f32();
        let t = (elapsed / total).clamp(0.0, 1.0);
        if t <= 0.0 {
            // the older picture's time is the picture before the newest,
            // not the newest one's
            return PictureKind::InBetween(0.0);
        }
        if t >= 1.0 {
            return PictureKind::Real;
        }
        PictureKind::InBetween(t)
    }
}

/// Optional host wall times. GPU waits inside a path are included in it.
#[derive(Default)]
pub struct HostTimings {
    pub compiled: HostSpan,
    pub interpreted: HostSpan,
    pub hooks: HostSpan,
    pub audio: HostSpan,
    pub frame: HostSpan,
    /// Nested inside system calls; do not add this to the other spans.
    pub gpu_commands: HostSpan,
    pub syscalls: BTreeMap<u32, HostSpan>,
}

#[derive(Default)]
pub struct HostSpan {
    pub calls: u64,
    pub elapsed: std::time::Duration,
}

impl HostSpan {
    fn record(&mut self, start: std::time::Instant) {
        self.calls += 1;
        self.elapsed += start.elapsed();
    }
}

pub struct System {
    pub cpu: Cpu,
    pub memory: Memory,
    pub kernel: Kernel,
    pub services: ServiceState,
    pub gpu: Gpu,
    pub renderer: Box<dyn Renderer>,
    pub title: Option<Title>,
    /// the title's downloadable content.
    pub dlc: Vec<zakuro_fs::Dlc>,
    /// the title's cheats, from its file in the data folder.
    pub cheats: Vec<cheats::Cheat>,
    pub config: Config,

    pub exited: bool,
    pub broke: bool,
    /// everything the title wrote with svcOutputDebugString.
    pub debug_output: String,
    pub unimplemented_svcs: BTreeSet<u32>,
    pub services_seen: BTreeSet<String>,
    pub lcd_force_black: bool,
    /// the scaled picture each screen had at the last presentation, which
    /// goes up at the next, once the GPU had a frame's time to draw it.
    showing: [Option<zakuro_gpu::ScreenRef>; 3],
    /// frame interpolation's count of the refreshes.
    pacing: Pacing,
    /// the dynamic module loader's state.
    pub cro: cro::CroManager,
    /// what the cameras see, from the frontend.
    pub camera: Option<Box<dyn camera::Camera>>,
    /// the room local wireless goes through, none with wireless off.
    pub room: Option<room::Link>,
    /// errors the title reported through err:f, newest last.
    pub fatal_errors: Vec<String>,
    /// mouse aiming for Kid Icarus: Uprising, when the frontend turned it on.
    pub aim_mouse: Option<aim_mouse::AimMouse>,
    /// undefined instructions we have seen, so the log stays readable.
    undefined_seen: BTreeMap<u32, u32>,
    /// branches into unmapped memory, keyed by target.
    pub prefetch_aborts: BTreeMap<u32, u32>,
    /// the last few instruction addresses, so a fault can say how it got there.
    history: [u32; HISTORY_LENGTH],
    history_index: usize,
    /// frames completed since boot.
    pub frames: u64,
    /// the host clock's time when the last frame finished, as a duration.
    /// a frontend driving a host clock of its own sets it, and boot's
    /// simulated host does; when it stays zero the clock counts real time.
    pub host_clock: std::time::Duration,
    /// the console's clock at boot, in milliseconds since 1900.
    pub(crate) boot_clock: u64,
    /// cycle count at which the next end-of-frame work is due.
    next_frame_boundary: u64,
    /// cycle count at which the DSP next finishes an audio frame.
    next_audio_frame: u64,
    /// cycle count at which the scheduler is next forced to run.
    next_preempt: u64,

    /// sampling profiler, counts how often each thread was found at each PC.
    pub profile: Option<BTreeMap<(ThreadId, u32), u64>>,
    pub host_timings: Option<HostTimings>,
    /// steps since the profiler last looked.
    profile_skip: u32,

    /// loops that change nothing, which the interpreter skips past.
    pub idle: idle::IdleLoops,

    /// code recompiled ahead of time, which runs instead of the interpreter
    /// wherever it has something.
    pub recompiled: Option<recompiled::Library>,
    /// where the title ran in the interpreter despite the library.
    pub hints: Option<hints::Hints>,
    /// instructions the interpreter ran for want of recompiled code, and
    /// the ones recompiled code ran, to see how much the library covers.
    pub interpreted_instructions: u64,
    pub recompiled_instructions: u64,
}

/// cycles in one frame of the screens, which refresh at 268111856 / 4481136,
/// about 59.83 Hz and not 60. a title that keeps time by frames runs that
/// much ahead of its sound otherwise, a rhythm game's notes drift off the
/// music over a song.
pub const CYCLES_PER_FRAME: u64 = 4_481_136;

/// cycles in one audio frame, 160 samples, each exactly 8192 cycles long
/// (the DSP's 32728 Hz is the CPU clock divided by 8192).
pub const CYCLES_PER_AUDIO_FRAME: u64 = 160 * 8192;
/// samples the DSP plays each second, one every 8192 cycles.
pub const AUDIO_SAMPLE_RATE: f64 = kernel::thread::CPU_CLOCK_HZ as f64 / 8192.0;

/// how often the scheduler is forced to run even if no thread yields.
pub const PREEMPT_INTERVAL: u64 = 8192;

/// how many instruction addresses are kept for fault reports.
const HISTORY_LENGTH: usize = 64;

/// what one call to [System::step] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    Ran,
    Exited,
    Faulted,
}

impl System {
    pub fn new(config: Config) -> System {
        let boot_clock = config.clock.unwrap_or_else(memory::config::host_clock);
        let idle = idle::IdleLoops::new(config.skip_idle_loops);
        let app_bytes = 64 * 1024 * 1024;
        let mut services = ServiceState::default();
        if let Some(dir) = &config.data_dir {
            services.fs.user_dir = dir.join("user");
        }
        System {
            cpu: Cpu::new(),
            memory: Memory::new(config.new3ds, app_bytes),
            kernel: Kernel::new(0, memory::MemoryRegion::Application, linear_heap_base(0)),
            services,
            gpu: Gpu::new(),
            renderer: Box::new(SoftwareRenderer::default()),
            title: None,
            dlc: Vec::new(),
            cheats: Vec::new(),
            config,
            exited: false,
            broke: false,
            debug_output: String::new(),
            unimplemented_svcs: BTreeSet::new(),
            services_seen: BTreeSet::new(),
            lcd_force_black: false,
            showing: [None, None, None],
            pacing: Pacing::default(),
            cro: cro::CroManager::default(),
            camera: None,
            room: None,
            fatal_errors: Vec::new(),
            aim_mouse: None,
            undefined_seen: BTreeMap::new(),
            prefetch_aborts: BTreeMap::new(),
            history: [0; HISTORY_LENGTH],
            history_index: 0,
            frames: 0,
            host_clock: std::time::Duration::ZERO,
            boot_clock,
            next_frame_boundary: CYCLES_PER_FRAME,
            next_audio_frame: CYCLES_PER_AUDIO_FRAME,
            next_preempt: PREEMPT_INTERVAL,
            profile: None,
            host_timings: None,
            profile_skip: 0,
            idle,
            recompiled: None,
            hints: None,
            interpreted_instructions: 0,
            recompiled_instructions: 0,
        }
    }

    // -- process setup ------------------------------------------------------

    /// maps the page holding a thread's TLS block, if it is not mapped yet,
    /// and clears the block, which a thread that ended may have used.
    pub fn map_tls_page(&mut self, thread: ThreadId) {
        let tls = self.kernel.thread(thread).tls;
        let page = tls & !PAGE_MASK;
        if self.memory.mapping_at(page).is_some() {
            self.memory.write_bytes(tls, &[0; TLS_ENTRY_SIZE as usize]);
            return;
        }
        let Some(block) = self
            .memory
            .phys
            .allocate(memory::MemoryRegion::Base, PAGE_SIZE)
        else {
            log::error!("out of memory allocating a TLS page");
            return;
        };
        self.memory.map(
            page,
            block.addr,
            PAGE_SIZE,
            Permission::RW,
            MemoryState::Locked,
        );
        // TLS must start zeroed, the IPC command buffer lives in it.
        self.memory.zero_physical(block.addr, PAGE_SIZE);
    }

    // -- the run loop -------------------------------------------------------

    /// runs one frame's worth of emulation.
    pub fn run_frame(&mut self) -> FrameOutcome {
        // HID samples its inputs on its own, whether or not anyone pressed
        // anything
        let input = self.services.hid.input;
        self.idle_interrupted(false);
        services::hid::update(self, input);
        cheats::run(self);
        // run up to the boundary step() would end the frame at, and end it
        // here, once, instead of again when the next frame's first step
        // finds the boundary passed
        let deadline = self.next_frame_boundary;
        while self.cpu.cycles < deadline {
            match self.step(Some(deadline)) {
                StepOutcome::Ran => {}
                StepOutcome::Exited => return FrameOutcome::Exited,
                StepOutcome::Faulted => return FrameOutcome::Faulted,
            }
        }
        self.next_frame_boundary = self.cpu.cycles + CYCLES_PER_FRAME;
        self.end_frame();
        FrameOutcome::Completed
    }

    /// advances the machine by one instruction, doing whatever scheduling is
    /// due first.
    pub fn step(&mut self, deadline: Option<u64>) -> StepOutcome {
        if self.exited {
            return StepOutcome::Exited;
        }
        if self.broke {
            return StepOutcome::Faulted;
        }

        // fire the end-of-frame work on schedule even when a tool is stepping,
        // or a title waiting on vertical blank would never be woken.
        if self.cpu.cycles >= self.next_frame_boundary {
            self.next_frame_boundary = self.cpu.cycles + CYCLES_PER_FRAME;
            self.end_frame();
        }
        if self.cpu.cycles >= self.next_audio_frame {
            self.next_audio_frame = self.cpu.cycles + CYCLES_PER_AUDIO_FRAME;
            self.audio_frame();
        }

        if self.kernel.reschedule_pending || self.kernel.current_thread.is_none() {
            self.kernel.reschedule_pending = false;
            let tick = self.cpu.cycles;
            if self.kernel.schedule(&mut self.cpu, tick) {
                self.idle_interrupted(true);
            }
            // a thread released here that keeps the core gets its result
            // too, before it runs on, not over whatever it does next
            self.apply_wait_result();
        }

        if self.kernel.current_thread.is_none() {
            // everything is blocked.
            let mut limit = deadline.unwrap_or(self.cpu.cycles + CYCLES_PER_FRAME);
            // an audio thread waits on the DSP's interrupt, which only fires
            // at an audio frame boundary, skipping past one would lose it.
            if self.services.dsp.running {
                limit = limit.min(self.next_audio_frame);
            }
            match self.kernel.next_event() {
                Some(tick) if tick > self.cpu.cycles => self.cpu.cycles = tick.min(limit),
                _ => self.cpu.cycles = limit,
            }
            self.kernel.reschedule_pending = true;
            return StepOutcome::Ran;
        }

        // a thread whose entry point returned branches to this address.
        if self.cpu.regs[15] == THREAD_EXIT_MAGIC {
            if let Some(id) = self.kernel.current_thread {
                self.kernel.end_thread(id);
            }
            return StepOutcome::Ran;
        }

        // a branch through an uninitialized function pointer lands on the null
        // page, whose zero words decode as harmless no-ops.
        let pc = self.cpu.regs[15];
        if !self.memory.is_executable(pc) {
            let count = self.prefetch_aborts.entry(pc).or_insert(0);
            *count += 1;
            if *count == 1 {
                let thread = self
                    .kernel
                    .current()
                    .map_or("?".to_owned(), |t| t.name.clone());
                log::error!(
                    "prefetch abort: {thread} branched to unmapped 0x{pc:08X} from 0x{:08X}",
                    self.cpu.regs[14]
                );
                log::error!("{:?}", self.cpu);
                log::error!("the instructions leading here were:");
                let recent = self.recent_instructions();
                for &entry in recent.iter().rev().take(8).rev() {
                    let address = entry & !1;
                    let mut bytes = [0u8; 8];
                    self.memory.read_bytes(address & !3, &mut bytes);
                    log::error!(
                        "  0x{address:08X} {} [{:02X?}]",
                        if entry & 1 != 0 { "T" } else { "A" },
                        bytes
                    );
                }
                self.fatal_errors.push(format!(
                    "branch to unmapped 0x{pc:08X} (lr 0x{:08X})",
                    self.cpu.regs[14]
                ));
            }
            // stop the thread rather than the machine, the rest of the title
            // may still make progress, and the report says what happened.
            if let Some(id) = self.kernel.current_thread {
                self.kernel.end_thread(id);
            }
            return StepOutcome::Ran;
        }

        if self.aim_mouse.is_some() {
            let start = self.host_timings.as_ref().map(|_| std::time::Instant::now());
            aim_mouse::apply(self, pc);
            if let (Some(timing), Some(start)) = (&mut self.host_timings, start) {
                timing.hooks.record(start);
            }
        }

        self.history[self.history_index] = pc | self.cpu.cpsr.thumb as u32;
        self.history_index = (self.history_index + 1) % HISTORY_LENGTH;

        if self.profile.is_some() {
            self.sample();
        }

        let exit = match self.run_recompiled(deadline) {
            Some(exit) => {
                // its writes do not go through the log a watch keeps
                self.idle_interrupted(false);
                if let Some(hints) = &mut self.hints {
                    hints.library_ran();
                }
                exit
            }
            None => {
                // a thread polling in a loop that changes nothing waits for
                // the next event, as it would on a console spinning freely
                if let Some(until) = self.idle.due(self.cpu.cycles).then(|| self.idle_until(deadline)).flatten() {
                    self.cpu.cycles = until;
                    self.kernel.reschedule_pending = true;
                    return StepOutcome::Ran;
                }
                self.interpreted_instructions += 1;
                let thumb = self.cpu.cpsr.thumb;
                if let (Some(hints), Some(library)) = (&mut self.hints, &self.recompiled) {
                    if !library.has_code(pc | thumb as u32) {
                        hints.interpreted(pc, thumb);
                    }
                }
                let gpu = &mut self.gpu as *mut Gpu as *mut ();
                let linear_base = self.kernel.linear_base;
                // SAFETY: the GPU stays where it is, and nothing but the CPU's
                // reads and writes follow the pointer, until the step is over
                unsafe { self.memory.set_gpu_sync(memory::GpuSync { gpu, linear_base, sync: sync_for_cpu, sync_write: sync_for_cpu_write }) };
                let start = self.host_timings.as_ref().map(|_| std::time::Instant::now());
                let exit = self.cpu.step(&mut self.memory);
                if let (Some(timing), Some(start)) = (&mut self.host_timings, start) {
                    timing.interpreted.record(start);
                }
                self.memory.clear_gpu_sync();
                exit
            }
        };
        if exit.is_some() {
            self.idle_interrupted(true);
        }
        match exit {
            None => {}
            Some(Exit::Supervisor(number)) => {
                let start = self.host_timings.as_ref().map(|_| std::time::Instant::now());
                kernel::svc::dispatch(self, number);
                if let (Some(timing), Some(start)) = (&mut self.host_timings, start) {
                    timing.syscalls.entry(number).or_default().record(start);
                }
            }
            Some(Exit::Undefined { pc, opcode }) => {
                let count = self.undefined_seen.entry(pc).or_insert(0);
                *count += 1;
                if *count == 1 {
                    log::error!("undefined instruction 0x{opcode:08X} at 0x{pc:08X}");
                    log::error!("{:?}", self.cpu);
                }
                // skip it and keep going, one bad decode should not end the
                // session while the CPU is still being filled in.
                let step = if self.cpu.cpsr.thumb { 2 } else { 4 };
                self.cpu.regs[15] = pc.wrapping_add(step);
            }
            Some(Exit::Breakpoint { pc, imm }) => {
                log::warn!("bkpt #{imm} at 0x{pc:08X}");
                let step = if self.cpu.cpsr.thumb { 2 } else { 4 };
                self.cpu.regs[15] = pc.wrapping_add(step);
            }
            Some(Exit::Halted) => {
                self.cpu.resume();
                self.kernel.reschedule_pending = true;
            }
            Some(Exit::Timeout) => {}
        }
        let _ = pc;

        // give the scheduler a chance on a regular cadence so that a thread
        // which never makes a syscall cannot monopolise the core.
        if self.cpu.cycles >= self.next_preempt {
            self.next_preempt = self.cpu.cycles + PREEMPT_INTERVAL;
            self.kernel.reschedule_pending = true;
        }

        if self.exited {
            StepOutcome::Exited
        } else if self.broke {
            StepOutcome::Faulted
        } else {
            StepOutcome::Ran
        }
    }

    /// runs recompiled code from the pc up to the next thing the scheduler
    /// has to look at, when the library has code there. what it returns
    /// stands in for what one interpreted step would.
    fn run_recompiled(&mut self, deadline: Option<u64>) -> Option<Option<Exit>> {
        let library = self.recompiled.as_ref()?;
        if !library.has_code(self.cpu.regs[15] | self.cpu.cpsr.thumb as u32) {
            return None;
        }
        let mut limit = self.next_frame_boundary.min(self.next_audio_frame);
        // a thread or a timer due wants the scheduler at its tick, a run
        // that reached it has it look first
        let event = self.kernel.next_event();
        if let Some(tick) = event {
            if tick <= self.cpu.cycles {
                self.kernel.reschedule_pending = true;
                return Some(None);
            }
            limit = limit.min(tick);
        }
        // a stop for the scheduler unwinds every guest call the code is in,
        // which the host then enters again one at a time. it only changes
        // anything when another thread can run
        if self.kernel.others_runnable() {
            limit = limit.min(self.next_preempt);
        }
        if let Some(deadline) = deadline {
            limit = limit.min(deadline);
        }
        let budget = limit.saturating_sub(self.cpu.cycles).max(1) * self.cpu.overclock as u64;
        // a read of the CPU from where the GPU drew has that come down
        // first, with nothing else holding the GPU until the run is over
        let gpu = &mut self.gpu as *mut Gpu as *mut ();
        let linear_base = self.kernel.linear_base;
        // SAFETY: as in step, until the run is over
        unsafe { self.memory.set_gpu_sync(memory::GpuSync { gpu, linear_base, sync: sync_for_cpu, sync_write: sync_for_cpu_write }) };
        let start = self.host_timings.as_ref().map(|_| std::time::Instant::now());
        let (ran, stop) = library.run(&mut self.cpu, &mut self.memory, budget);
        if let (Some(timing), Some(start)) = (&mut self.host_timings, start) {
            timing.compiled.record(start);
        }
        self.memory.clear_gpu_sync();
        self.recompiled_instructions += ran;
        if event.is_some_and(|tick| self.cpu.cycles >= tick) {
            self.kernel.reschedule_pending = true;
        }
        if ran == 0 && matches!(stop, recompiled::Stop::Left) {
            // not enough budget left for a whole block
            if let Some(hints) = &mut self.hints {
                hints.library_declined();
            }
            return None;
        }
        Some(match stop {
            recompiled::Stop::Svc(number) => Some(Exit::Supervisor(number)),
            recompiled::Stop::Exit(exit) => Some(exit),
            recompiled::Stop::Left => None,
        })
    }

    /// the instruction addresses executed most recently, oldest first.
    pub fn recent_instructions(&self) -> Vec<u32> {
        let mut out = Vec::with_capacity(HISTORY_LENGTH);
        for offset in 0..HISTORY_LENGTH {
            let entry = self.history[(self.history_index + offset) % HISTORY_LENGTH];
            if entry != 0 {
                out.push(entry);
            }
        }
        out
    }

    /// starts collecting PC samples.
    pub fn enable_profiler(&mut self) {
        self.profile = Some(BTreeMap::new());
    }

    /// the hottest sampled locations, most frequent first.
    pub fn hot_spots(&self, count: usize) -> Vec<(String, u32, u64)> {
        let Some(profile) = &self.profile else {
            return Vec::new();
        };
        let mut entries: Vec<(String, u32, u64)> =
            profile.iter().map(|(&(thread, pc), &hits)| (self.thread_name(thread), pc, hits)).collect();
        entries.sort_by_key(|&(_, _, hits)| std::cmp::Reverse(hits));
        entries.truncate(count);
        entries
    }

    /// how many samples found each thread running, most first.
    pub fn profiled_threads(&self) -> Vec<(String, u64)> {
        let Some(profile) = &self.profile else {
            return Vec::new();
        };
        let mut threads: BTreeMap<ThreadId, u64> = BTreeMap::new();
        for (&(thread, _), &hits) in profile {
            *threads.entry(thread).or_insert(0) += hits;
        }
        let mut threads: Vec<(String, u64)> = threads.into_iter().map(|(thread, hits)| (self.thread_name(thread), hits)).collect();
        threads.sort_by_key(|&(_, hits)| std::cmp::Reverse(hits));
        threads
    }

    fn thread_name(&self, id: ThreadId) -> String {
        self.kernel.threads.get(id as usize).map_or_else(|| format!("thread {id}"), |thread| thread.name.clone())
    }

    fn sample(&mut self) {
        // one step in so many, a prime so it does not beat in step with a
        // loop, keeps a profile from slowing the run down
        self.profile_skip = self.profile_skip.wrapping_add(1);
        if !self.profile_skip.is_multiple_of(61) {
            return;
        }
        let Some(id) = self.kernel.current_thread else {
            return;
        };
        let pc = self.cpu.regs[15];
        if let Some(profile) = &mut self.profile {
            *profile.entry((id, pc)).or_insert(0) += 1;
        }
    }

    /// writes back the result registers of a syscall that had blocked.
    fn apply_wait_result(&mut self) {
        let Some(id) = self.kernel.current_thread else {
            return;
        };
        let thread = self.kernel.thread_mut(id);
        let (Some(result), Some(syscall)) = (thread.wait_result.take(), thread.wait_syscall.take())
        else {
            return;
        };

        match (syscall, result) {
            (WaitSyscall::WaitSynchronization1, WaitResult::Signaled(_)) => {
                self.cpu.regs[0] = 0;
            }
            (WaitSyscall::WaitSynchronizationN, WaitResult::Signaled(index)) => {
                self.cpu.regs[0] = 0;
                self.cpu.regs[1] = index as u32;
            }
            (WaitSyscall::ArbitrateAddress, WaitResult::Signaled(_)) => {
                self.cpu.regs[0] = 0;
            }
            (_, WaitResult::TimedOut) => {
                self.cpu.regs[0] = zakuro_common::result::errors::TIMEOUT.0;
            }
            (WaitSyscall::SleepThread, _) => {
                self.cpu.regs[0] = 0;
            }
        }
    }

    /// everything that happens between frames, vertical blank, input, clock.
    fn end_frame(&mut self) {
        let start = self.host_timings.as_ref().map(|_| std::time::Instant::now());
        self.idle_interrupted(false);
        self.frames += 1;
        if self.frames.is_multiple_of(hints::SAVE_EVERY) {
            if let Some(hints) = &mut self.hints {
                hints.save();
            }
        }

        // refresh the kernel's shared page so the guest's clock advances.
        let tick = self.cpu.cycles;
        memory::config::update_datetime(self.memory.phys.shared_page_mut(), self.boot_clock, tick);

        // gather the timers again, without the ones closed since. they fire
        // when the scheduler gets to their tick
        self.kernel.timers.clear();
        for (id, object) in self.kernel.objects.iter() {
            if let kernel::object::KObject::Timer(_) = object {
                self.kernel.timers.push(id);
            }
        }

        self.renderer.end_frame();

        // pick up any buffer swap the game queued directly in GSP shared
        // memory before the LCDs latch whatever is currently configured.
        services::gsp::refresh(self);
        // the top screen's new pictures are counted at every refresh, and
        // High Refresh Mode paces the ones in between.
        self.pace();

        // both LCDs finish scanning out, in that order.
        services::gsp::signal_interrupt(self, services::gsp::InterruptId::Pdc0);
        services::gsp::signal_interrupt(self, services::gsp::InterruptId::Pdc1);
        if let (Some(timing), Some(start)) = (&mut self.host_timings, start) {
            timing.frame.record(start);
        }
    }

    /// the top screen's picture at a console refresh. a new one is counted,
    /// with the host time it arrived at, for the game's frame rate and for
    /// the pictures in between. while High Refresh Mode is on and a title
    /// sets a new picture every refresh, or every other one, the GPU draws a
    /// picture in between along with each, from the frame before's draws and
    /// this one's, see zakuro_gpu's interpolation. the host's presenter
    /// picks the title's own or one in between at the host time it shows it
    /// at. a title that stalled, more than two refreshes without a new
    /// picture, draws none in between.
    fn pace(&mut self) {
        let pacing = &mut self.pacing;
        pacing.refreshes += 1;
        pacing.between = false;
        let top = self.screen_ref(zakuro_common::Screen::Top, false);
        let pacing = &mut self.pacing;
        if top.is_none() || top == pacing.last {
            if pacing.on && pacing.refreshes - pacing.new_at > 2 {
                // the title slowed down or stopped, the pictures in between
                // of its next frame would be shown too late
                self.gpu.set_twins(false);
            }
            return;
        }
        let gap = pacing.refreshes - pacing.new_at;
        pacing.new_at = pacing.refreshes;
        pacing.last = top;
        pacing.pictures += 1;
        // the host times the two newest pictures arrived at, which a picture
        // in between is drawn at a fraction of. a title drawing every other
        // refresh keeps its two-refresh interval, rather than the console's
        // one-refresh interval between its frames.
        pacing.older_host_time = pacing.newer_host_time;
        pacing.newer_host_time = Some(self.host_clock);
        if !pacing.on {
            return;
        }
        let matches = self.gpu.frame_done();
        let pacing = &mut self.pacing;
        pacing.matches = matches;
        // High Refresh Mode shows the pictures in between at every refresh
        // of the host, so a title drawing every refresh keeps its twins too,
        // as does one drawing every other refresh
        pacing.between = gap <= 2 && matches.is_some_and(|matches| matches.smooth());
        pacing.shown += pacing.between as u64;
        self.gpu.set_twins(gap <= 2);
    }

    /// draws a picture in between each two of the title's own from now on,
    /// for High Refresh Mode, see pace, or stops. it starts the pacer and
    /// the matcher over, so no picture in between comes from before the
    /// switch. it says whether it does, it takes the host's GPU showing the
    /// screens.
    pub fn set_interpolation(&mut self, on: bool) -> bool {
        self.pacing = Pacing::default();
        let on = self.gpu.set_interpolation(on);
        // the pacer decides the pictures only while the mode is really on,
        // which takes the host's GPU showing the screens
        self.pacing.on = on;
        on
    }

    /// how the draws of the last frame matched the frame before's, for frame
    /// interpolation.
    pub fn interpolation_matches(&self) -> Option<zakuro_gpu::Matches> {
        self.pacing.matches
    }

    /// whether the screens show the pictures in between at this refresh.
    pub fn shows_between(&self) -> bool {
        self.pacing.between && !self.real_frames_only()
    }

    /// how many new pictures the top screen has shown, which is the game's
    /// own frame rate counted in pictures: a title drawing every other
    /// refresh has one every other console refresh.
    pub fn pictures(&self) -> u64 {
        self.pacing.pictures
    }

    /// the DSP finishing an audio frame, it plays its voices one frame on and
    /// interrupts the title, whose sound library does one update per interrupt.
    fn audio_frame(&mut self) {
        let start = self.host_timings.as_ref().map(|_| std::time::Instant::now());
        self.idle_interrupted(false);
        // the whole opening froze until this ran every 4.9 ms. goddamn audio timing
        if self.services.dsp.running {
            services::dsp::advance(self);
            services::dsp::signal_semaphore(self);
        }
        // the devices that report at a rate a title sets, which this is
        // fine enough for
        services::ir_rst::tick(self);
        services::ir::tick(self);
        services::cam::tick(self);
        services::uds::tick(self);
        services::dlp::tick(self);
        if let (Some(timing), Some(start)) = (&mut self.host_timings, start) {
            timing.audio.record(start);
        }
    }

    /// the stereo samples the console played since the last call, at
    /// AUDIO_SAMPLE_RATE.
    pub fn take_audio(&mut self) -> Vec<[i16; 2]> {
        std::mem::take(&mut self.services.dsp.output)
    }

    /// what the software keyboard asks for, while a title waits on it.
    pub fn keyboard_request(&self) -> Option<&services::keyboard::Request> {
        services::keyboard::request(self)
    }

    /// closes the software keyboard with the text typed and the button
    /// pressed, zero being the leftmost and the last one confirming.
    pub fn answer_keyboard(&mut self, text: &str, button: usize) {
        services::keyboard::answer(self, text, button);
    }

    /// what the buttons, circle pad and touch screen are doing, which HID
    /// reports from the next frame on.
    pub fn set_input(&mut self, input: InputState) {
        self.services.hid.input = input;
    }

    // -- GPU plumbing -------------------------------------------------------

    pub fn gpu_read_register(&mut self, offset: u32) -> u32 {
        self.gpu.read_external(offset)
    }

    pub fn gpu_write_register(&mut self, offset: u32, value: u32) {
        self.gpu.write_external(offset, value);
    }

    pub fn set_framebuffer(
        &mut self,
        screen: u32,
        active: u32,
        left: u32,
        right: u32,
        stride: u32,
        format: u32,
    ) {
        self.gpu
            .set_framebuffer(screen, active, left, right, stride, format);
    }

    pub fn submit_command_list(&mut self, paddr: u32, size: u32) {
        let start = self.host_timings.as_ref().map(|_| std::time::Instant::now());
        let linear_base = self.kernel.linear_base;
        let mut guest = GuestMemory {
            linear_base,
            memory: &mut self.memory,
        };
        self.gpu
            .process_command_list(&mut guest, self.renderer.as_mut(), paddr, size);
        if let (Some(timing), Some(start)) = (&mut self.host_timings, start) {
            timing.gpu_commands.record(start);
        }
    }

    /// makes guest memory hold what the host GPU drew over a range, before
    /// something other than a draw reads or writes it.
    pub fn sync_gpu(&mut self, addr: u32, len: u32) {
        let linear_base = self.kernel.linear_base;
        let mut guest = GuestMemory {
            linear_base,
            memory: &mut self.memory,
        };
        self.gpu.sync_memory(&mut guest, addr, len);
    }

    /// writes what a service hands the title, a file it read among them,
    /// over guest memory, after what the host GPU drew there. on the console
    /// the drawing was in memory long before, and the title can have put
    /// the buffer where one it was done drawing to was.
    pub fn write_from_service(&mut self, addr: u32, data: &[u8]) {
        if !data.is_empty() {
            self.sync_gpu(addr, data.len() as u32);
        }
        self.memory.write_bytes(addr, data);
    }

    pub fn memory_fill(&mut self, start: u32, end: u32, value: u32, width: u32) {
        let linear_base = self.kernel.linear_base;
        let mut guest = GuestMemory {
            linear_base,
            memory: &mut self.memory,
        };
        self.gpu.memory_fill(&mut guest, start, end, value, width);
    }

    pub fn display_transfer(
        &mut self,
        input: u32,
        output: u32,
        input_dimensions: u32,
        output_dimensions: u32,
        flags: u32,
    ) {
        let linear_base = self.kernel.linear_base;
        let mut guest = GuestMemory {
            linear_base,
            memory: &mut self.memory,
        };
        self.gpu.display_transfer(
            &mut guest,
            input,
            output,
            input_dimensions,
            output_dimensions,
            flags,
        );
    }

    pub fn texture_copy(
        &mut self,
        input: u32,
        output: u32,
        size: u32,
        input_gap: u32,
        output_gap: u32,
    ) {
        let linear_base = self.kernel.linear_base;
        let mut guest = GuestMemory {
            linear_base,
            memory: &mut self.memory,
        };
        self.gpu
            .texture_copy(&mut guest, input, output, size, input_gap, output_gap);
    }

    /// the nine words gsp::ImportDisplayCaptureInfo returns, the addresses
    /// virtual, as the title gave them.
    pub fn display_capture_info(&mut self) -> [u32; 9] {
        let top = self.gpu.framebuffers[0];
        let bottom = self.gpu.framebuffers[1];
        let address = |paddr: u32| if paddr == 0 { 0 } else { services::gsp::physical_to_virtual(self, paddr) };
        [
            address(top.address_left()),
            address(top.address_right()),
            top.format,
            top.stride,
            address(bottom.address_left()),
            address(bottom.address_right()),
            bottom.format,
            bottom.stride,
            0,
        ]
    }

    /// a screen as RGBA at the resolution the host's GPU draws at, and its
    /// width and height. a picture the GPU did not draw, or the CPU changed
    /// since, is the console's own grown to it, so the size stays the same
    /// from frame to frame.
    pub fn read_screen_scaled(&mut self, screen: zakuro_common::Screen) -> Screen {
        self.read_scaled(screen, false)
    }

    /// the top screen's picture for the right eye, as read_screen_scaled
    /// gives the left one's, when the title draws in 3D.
    pub fn read_right_eye_scaled(&mut self) -> Option<Screen> {
        self.draws_in_3d().then(|| self.read_scaled(zakuro_common::Screen::Top, true))
    }

    fn read_scaled(&mut self, screen: zakuro_common::Screen, right: bool) -> Screen {
        let (width, height) = (screen.width(), screen.height());
        let scale = self.gpu.scale();
        if let Some(scaled) = self.scaled_screen(screen, right) {
            return scaled;
        }
        let native = self.read_screen_at(screen, right);
        if scale == 1 {
            return (std::sync::Arc::new(native), width, height);
        }
        (std::sync::Arc::new(grow(&native, width, scale)), width * scale, height * scale)
    }

    /// whether the title draws the top screen in 3D, a picture for each
    /// eye. its format asks for the screen's two buffers, the second for
    /// the right eye, which the console's GSP turns the 3D on for only
    /// with bit 5 set and bit 6 clear.
    pub fn draws_in_3d(&self) -> bool {
        let config = self.gpu.framebuffers[0];
        config.format & 0x60 == 0x20 && config.address_right() != 0
    }

    /// moves the 3D slider, which titles read every frame to decide whether
    /// and how deep to draw in 3D, 0 down and 1 all the way up.
    pub fn set_slider_3d(&mut self, value: f32) {
        self.config.slider_3d = value;
        memory::config::set_slider_3d(self.memory.phys.shared_page_mut(), value);
    }

    /// where a screen's picture is on the host's GPU, for a presenter sharing
    /// its device to draw straight from, none when the GPU does not show the
    /// screens itself or did not draw what the screen shows, read_screen_scaled
    /// has that. the presenter's work runs after the GPU draws the picture, so
    /// the newest goes up.
    pub fn gpu_screen(&mut self, screen: zakuro_common::Screen) -> Option<zakuro_gpu::GpuScreen> {
        self.gpu_picture(screen, false)
    }

    /// the kind of picture a screen shows at a host time, without the picture:
    /// the title's own at a frame's time, or one in between when the title
    /// draws every other refresh and the host time falls between its frames.
    /// a title that stalled, more than two refreshes without a new picture,
    /// or a frontend with interpolation off, shows its own.
    pub fn picture_kind_at_host_time(&self, host_time: std::time::Duration) -> PictureKind {
        if !self.pacing.on || self.real_frames_only() {
            return PictureKind::Real;
        }
        self.pacing.kind_at_host_time(host_time)
    }

    /// whether the Screen on the top is capped at 60, the title's own frames
    /// only. A Screen found to misbehave under interpolation takes the cap
    /// instead of being excluded from High Refresh Mode by category.
    fn real_frames_only(&self) -> bool {
        self.aim_mouse.as_ref().is_some_and(|mouse| mouse.real_frames_only(self.frames))
    }

    /// a screen's picture for an eye at a given kind, a screen's own or one
    /// in between. the picture in between is the GPU's when it draws the
    /// screens straight from its device, the title's own otherwise.
    fn picture_of_kind(&mut self, screen: zakuro_common::Screen, right: bool, kind: PictureKind) -> Screen {
        match kind {
            PictureKind::Real => self.read_scaled(screen, right),
            PictureKind::InBetween(_) => self.upright_screen_at(screen, right, true).unwrap_or_else(|| self.read_scaled(screen, right)),
        }
    }

    /// a screen's picture at a host time, and whether it is the title's own or
    /// one in between. the picture in between is the GPU's when it draws the
    /// screens straight from its device, the title's own otherwise.
    pub fn picture_at_host_time(&mut self, screen: zakuro_common::Screen, host_time: std::time::Duration) -> HostPicture {
        let kind = self.picture_kind_at_host_time(host_time);
        let picture = self.picture_of_kind(screen, false, kind);
        HostPicture { kind, screen: picture }
    }

    /// the top screen's pictures for both eyes at a host time, and whether
    /// they are the title's own or one in between. the one kind is decided
    /// here for the pair, so both eyes are the same picture at the same t,
    /// and a 3D picture never tears between them. the right eye's picture is
    /// none when the title does not draw in 3D.
    pub fn stereo_picture_at_host_time(&mut self, host_time: std::time::Duration) -> StereoPicture {
        let kind = self.picture_kind_at_host_time(host_time);
        let left = HostPicture { kind, screen: self.picture_of_kind(zakuro_common::Screen::Top, false, kind) };
        let right = if self.draws_in_3d() {
            Some(HostPicture { kind, screen: self.picture_of_kind(zakuro_common::Screen::Top, true, kind) })
        } else {
            None
        };
        StereoPicture { left, right }
    }

    /// a screen's picture at a host time on the host's GPU, and whether it is
    /// the title's own or one in between, for a presenter sharing the device
    /// to draw straight from. none when the GPU does not show the screens
    /// itself or did not draw what the screen shows.
    pub fn gpu_picture_at_host_time(&mut self, screen: zakuro_common::Screen, right: bool, host_time: std::time::Duration) -> (Option<zakuro_gpu::GpuScreen>, PictureKind) {
        let kind = self.picture_kind_at_host_time(host_time);
        (self.gpu_picture_of_kind(screen, right, kind), kind)
    }

    /// the top screen's pictures for both eyes at a host time on the host's
    /// GPU, and whether they are the title's own or one in between, for a
    /// presenter sharing the device to draw straight from. the one kind is
    /// decided here for the pair, so both eyes are the same picture at the
    /// same t, as stereo_picture_at_host_time has it, and neither can be
    /// drawn in between at a t the other is not.
    pub fn gpu_stereo_picture_at_host_time(&mut self, host_time: std::time::Duration) -> GpuStereoPicture {
        let kind = self.picture_kind_at_host_time(host_time);
        let left = self.gpu_picture_of_kind(zakuro_common::Screen::Top, false, kind);
        let right = if self.draws_in_3d() { self.gpu_picture_of_kind(zakuro_common::Screen::Top, true, kind) } else { None };
        GpuStereoPicture { left, right, kind }
    }

    /// a screen's picture for an eye at a given kind, as a presenter sharing
    /// the GPU's device draws it. both eyes take this same path, the eye
    /// only choosing which buffer's picture it is.
    fn gpu_picture_of_kind(&mut self, screen: zakuro_common::Screen, right: bool, kind: PictureKind) -> Option<zakuro_gpu::GpuScreen> {
        match kind {
            PictureKind::Real => self.gpu_picture_real(screen, right),
            PictureKind::InBetween(_) => {
                if !self.gpu.shows_directly() {
                    return None;
                }
                let now = self.screen_ref(screen, right)?;
                match self.gpu.between_image(now) {
                    Some(between) => Some(between),
                    None => self.gpu.screen_image(now),
                }
            }
        }
    }

    /// the top screen's picture for the right eye on the host's GPU, as
    /// gpu_screen gives the left one's, when the title draws in 3D.
    pub fn gpu_right_eye(&mut self) -> Option<zakuro_gpu::GpuScreen> {
        if !self.draws_in_3d() {
            return None;
        }
        self.gpu_picture(zakuro_common::Screen::Top, true)
    }

    /// a screen's own newest picture on the host's GPU, never one drawn in
    /// between, for fast forward showing the title's frames only.
    pub fn gpu_screen_real(&mut self, screen: zakuro_common::Screen) -> Option<zakuro_gpu::GpuScreen> {
        self.gpu_picture_real(screen, false)
    }

    /// the top screen's own newest picture for the right eye, never one
    /// drawn in between.
    pub fn gpu_right_eye_real(&mut self) -> Option<zakuro_gpu::GpuScreen> {
        if !self.draws_in_3d() {
            return None;
        }
        self.gpu_picture_real(zakuro_common::Screen::Top, true)
    }

    /// the newest picture the host's GPU drew for a screen, never the one it
    /// drew in between it and the one before.
    fn gpu_picture_real(&mut self, screen: zakuro_common::Screen, right: bool) -> Option<zakuro_gpu::GpuScreen> {
        if !self.gpu.shows_directly() {
            return None;
        }
        let now = self.screen_ref(screen, right)?;
        self.gpu.screen_image(now)
    }

    fn gpu_picture(&mut self, screen: zakuro_common::Screen, right: bool) -> Option<zakuro_gpu::GpuScreen> {
        if !self.gpu.shows_directly() {
            return None;
        }
        let now = self.screen_ref(screen, right)?;
        // frame interpolation's picture in between, where the screen has one
        // and the Screen on it may be drawn in between
        if self.pacing.between && !self.real_frames_only() {
            if let Some(between) = self.gpu.between_image(now) {
                return Some(between);
            }
        }
        self.gpu.screen_image(now)
    }

    /// a screen's picture as the GPU showing the screens has it, or the
    /// picture in between it and the one before, see pace, for saving it.
    /// the left eye's of the top screen. it waits for the GPU.
    pub fn upright_screen(&mut self, screen: zakuro_common::Screen, between: bool) -> Option<Screen> {
        self.upright_screen_at(screen, false, between)
    }

    /// the same, for the eye given, so each eye of a stereo picture can be
    /// saved as the picture the GPU drew for it.
    fn upright_screen_at(&mut self, screen: zakuro_common::Screen, right: bool, between: bool) -> Option<Screen> {
        let now = self.screen_ref(screen, right)?;
        let (image, scale) = self.gpu.upright_picture(now, between)?;
        Some((image, screen.width() * scale, screen.height() * scale))
    }

    /// the picture the host's GPU drew scaled for a screen, when it is still
    /// what the screen shows.
    fn scaled_screen(&mut self, screen: zakuro_common::Screen, right: bool) -> Option<Screen> {
        let (width, height) = (screen.width(), screen.height());
        let index = match screen {
            _ if right => 2,
            zakuro_common::Screen::Top => 0,
            zakuro_common::Screen::Bottom => 1,
        };
        let Some(now) = self.screen_ref(screen, right) else {
            self.showing[index] = None;
            return None;
        };
        // what the screen had a presentation ago goes up now, always that
        // one, so each picture stays up as long as the title kept it
        let show = self.showing[index].replace(now).unwrap_or(now);
        let (image, scale) = self.gpu.scaled_picture(show).or_else(|| self.gpu.scaled_picture(now))?;
        Some((image, width * scale, height * scale))
    }

    /// the picture the host's GPU drew for a screen, when it is still what
    /// the screen shows, the top one's for the right eye when right.
    fn screen_ref(&mut self, screen: zakuro_common::Screen, right: bool) -> Option<zakuro_gpu::ScreenRef> {
        let (width, height) = (screen.width(), screen.height());
        let index = match screen {
            zakuro_common::Screen::Top => 0,
            zakuro_common::Screen::Bottom => 1,
        };
        let config = self.gpu.framebuffers[index];
        let address = if right { config.address_right() } else { config.address_left() };
        let format = config.color_format();
        let bpp = format.bytes_per_pixel() as u32;
        // plain buffers a row per screen column, as display transfers leave
        // them, rows can be longer than the screen
        let stride = if config.stride == 0 { height * bpp } else { config.stride };
        if self.lcd_force_black || address == 0 || !stride.is_multiple_of(bpp) || stride < height * bpp {
            return None;
        }
        // no sync first, that would wait for the GPU to finish the picture.
        // compared where it lies, a copy of both screens every frame costs
        // more than the comparing
        let base = services::gsp::physical_to_virtual(self, address);
        let len = width * stride;
        let mut guest = GuestMemory { linear_base: self.kernel.linear_base, memory: &mut self.memory };
        if let Some(bytes) = guest.slice(base, len as usize) {
            return self.gpu.scaled_screen(base, (height, width), stride / bpp, format, bytes);
        }
        let mut bytes = vec![0u8; len as usize];
        self.memory.read_bytes(base, &mut bytes);
        self.gpu.scaled_screen(base, (height, width), stride / bpp, format, &bytes)
    }

    /// reads one screen into a straight RGBA8 buffer for presentation.
    pub fn read_screen(&mut self, screen: zakuro_common::Screen) -> Vec<u8> {
        self.read_screen_at(screen, false)
    }

    /// the same, the top screen's picture for the right eye when right.
    fn read_screen_at(&mut self, screen: zakuro_common::Screen, right: bool) -> Vec<u8> {
        let index = match screen {
            zakuro_common::Screen::Top => 0,
            zakuro_common::Screen::Bottom => 1,
        };
        let config = self.gpu.framebuffers[index];
        let width = screen.width();
        let height = screen.height();
        let mut out = vec![0u8; (width * height * 4) as usize];

        let address = if right { config.address_right() } else { config.address_left() };
        if self.lcd_force_black || address == 0 {
            return out;
        }

        let format = config.color_format();
        let bpp = format.bytes_per_pixel();
        let stride = if config.stride == 0 {
            height * bpp as u32
        } else {
            config.stride
        };

        let base = services::gsp::physical_to_virtual(self, address);
        let source_len = (stride * width) as usize;
        self.sync_gpu(base, source_len as u32);
        let mut source = vec![0u8; source_len];
        self.memory.read_bytes(base, &mut source);

        for x in 0..width {
            for y in 0..height {
                // column-major in memory, and the panel scans bottom to top.
                let offset = (x * stride + (height - 1 - y) * bpp as u32) as usize;
                if offset + bpp > source.len() {
                    continue;
                }
                let pixel = format.decode(&source[offset..offset + bpp]);
                let dst = ((y * width + x) * 4) as usize;
                out[dst..dst + 4].copy_from_slice(&pixel);
            }
        }
        out
    }

    /// a one-line summary for the window title and the log.
    pub fn status_line(&self) -> String {
        let mut line = format!(
            "frame {} | {} threads | {} modules | {} draws | {} transfers | {} fills | gpu {:.1?}",
            self.frames,
            self.kernel.live_thread_count(),
            self.cro.len(),
            self.gpu.draw_calls,
            self.gpu.transfers,
            self.gpu.fills,
            self.gpu.busy,
        );
        if self.gpu.interpolates() {
            line += &format!(" | in between {}", self.pacing.shown);
        }
        if let Some(library) = &self.recompiled {
            // the fallbacks ran inside recompiled code, which counted them
            let interpreted = self.interpreted_instructions + library.fallbacks();
            let total = (self.interpreted_instructions + self.recompiled_instructions).max(1);
            line += &format!(" | interpreted {:.2}%", interpreted as f64 * 100.0 / total as f64);
        }
        line
    }
}

/// adapter letting the GPU reach guest memory by physical address.
struct GuestMemory<'a> {
    memory: &'a mut Memory,
    linear_base: u32,
}

impl GpuMemory for GuestMemory<'_> {
    fn read(&mut self, addr: u32, out: &mut [u8]) {
        self.memory.read_bytes(addr, out);
    }

    fn write(&mut self, addr: u32, data: &[u8]) {
        self.memory.write_bytes(addr, data);
    }

    fn read_u32(&mut self, addr: u32) -> u32 {
        // not as the CPU reads, which could have the GPU write back from
        // inside itself
        self.memory.peek32(addr)
    }

    fn guard(&mut self, addr: u32, len: u32) {
        self.memory.guard_cpu_reads(addr, len);
    }

    fn guard_writes(&mut self, addr: u32, len: u32) {
        self.memory.guard_cpu_writes(addr, len);
    }

    fn slice(&mut self, addr: u32, len: usize) -> Option<&[u8]> {
        self.slice_mut(addr, len).map(|slice| &*slice)
    }

    fn slice_mut(&mut self, addr: u32, len: usize) -> Option<&mut [u8]> {
        // the linear heap and VRAM are physical memory in order, so what
        // translate made of a physical address leads back to it. VRAM first,
        // from the old linear heap's base a New 3DS's heap size reaches it
        let physical = if (VRAM_VADDR..VRAM_VADDR + VRAM_SIZE).contains(&addr) {
            VRAM_PADDR + (addr - VRAM_VADDR)
        } else if addr >= self.linear_base && addr - self.linear_base < FCRAM_SIZE_NEW3DS {
            FCRAM_PADDR + (addr - self.linear_base)
        } else {
            return None;
        };
        self.memory.phys.host_slice_mut(physical, u32::try_from(len).ok()?)
    }

    fn translate(&self, paddr: u32) -> u32 {
        if paddr >= FCRAM_PADDR {
            self.linear_base + (paddr - FCRAM_PADDR)
        } else if (VRAM_PADDR..VRAM_PADDR + VRAM_SIZE).contains(&paddr) {
            VRAM_VADDR + (paddr - VRAM_PADDR)
        } else {
            paddr
        }
    }
}

/// writes back what the GPU drew over a range, for a read of the CPU
/// waiting on it.
///
/// # Safety
///
/// gpu has to be the system's GPU, which nothing else holds while the CPU
/// runs.
unsafe fn sync_for_cpu(gpu: *mut (), linear_base: u32, memory: &mut Memory, addr: u32, len: u32) {
    log::debug!(
        target: "zakuro_core::memory",
        "the CPU reads 0x{addr:08X}..0x{:08X}, which the GPU drew, written back first",
        addr.wrapping_add(len)
    );
    let gpu = unsafe { &mut *(gpu as *mut Gpu) };
    gpu.sync_depth(&mut GuestMemory { memory, linear_base }, addr, len);
}

/// writes back what the GPU drew over a range, for a write of the CPU
/// landing over it.
///
/// # Safety
///
/// as sync_for_cpu.
unsafe fn sync_for_cpu_write(gpu: *mut (), linear_base: u32, memory: &mut Memory, addr: u32, len: u32) {
    log::debug!(
        target: "zakuro_core::memory",
        "the CPU writes 0x{addr:08X}..0x{:08X}, which the GPU drew, written back first",
        addr.wrapping_add(len)
    );
    let gpu = unsafe { &mut *(gpu as *mut Gpu) };
    gpu.sync_memory(&mut GuestMemory { memory, linear_base }, addr, len);
}

impl Drop for System {
    /// what the interpreter ran lasts past the run.
    fn drop(&mut self) {
        if let Some(hints) = &mut self.hints {
            hints.save();
        }
    }
}

/// an RGBA picture of a given width grown by a whole factor, each pixel
/// repeated.
fn grow(pixels: &[u8], width: u32, scale: u32) -> Vec<u8> {
    let (width, scale) = (width as usize, scale as usize);
    let mut out = Vec::with_capacity(pixels.len() * scale * scale);
    for row in pixels.chunks_exact(width * 4) {
        let mut grown = Vec::with_capacity(row.len() * scale);
        for pixel in row.as_chunks::<4>().0 {
            for _ in 0..scale {
                grown.extend_from_slice(pixel);
            }
        }
        for _ in 0..scale {
            out.extend_from_slice(&grown);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::object::KObject;
    use kernel::thread::ThreadStatus;
    use zakuro_cpu::Bus;

    fn mutex_state(system: &System, handle: u32) -> (Option<ThreadId>, u32) {
        let object = system.kernel.handles.resolve(handle).expect("a mutex handle");
        match system.kernel.objects.get(object) {
            Some(KObject::Mutex(mutex)) => (mutex.owner, mutex.lock_count),
            _ => panic!("not a mutex"),
        }
    }

    /// a holder that took a mutex twice, through svcCreateMutex locked and
    /// svcWaitSynchronization1, and a waiter blocked on it, with the holder
    /// back on the core.
    fn holder_and_waiter(system: &mut System) -> (ThreadId, ThreadId, u32) {
        let holder = system.kernel.create_thread("holder", 0x0010_0000, 0x1000_0000, 0, 0x30, 0);
        let waiter = system.kernel.create_thread("waiter", 0x0010_0000, 0x0FF0_0000, 0, 0x30, 0);
        system.map_tls_page(holder);
        system.map_tls_page(waiter);
        system.kernel.schedule(&mut system.cpu, 0);
        assert_eq!(system.kernel.current_thread, Some(holder));
        system.cpu.regs[1] = 1;
        kernel::svc::dispatch(system, 0x13);
        let handle = system.cpu.regs[1];
        let wait = |system: &mut System| {
            system.cpu.regs[0] = handle;
            system.cpu.regs[2] = u32::MAX;
            system.cpu.regs[3] = u32::MAX;
            kernel::svc::dispatch(system, 0x24);
        };
        wait(system);
        assert_eq!(mutex_state(system, handle), (Some(holder), 2));
        system.kernel.current_thread = Some(waiter);
        system.kernel.thread_mut(waiter).status = ThreadStatus::Running;
        system.kernel.thread_mut(holder).status = ThreadStatus::Ready;
        wait(system);
        assert_eq!(system.kernel.thread(waiter).status, ThreadStatus::WaitSync);
        system.kernel.current_thread = Some(holder);
        system.kernel.thread_mut(holder).status = ThreadStatus::Running;
        system.kernel.reschedule_pending = false;
        (holder, waiter, handle)
    }

    /// the holder ended with the mutex all given back, and the waiter gets it
    /// once at the next schedule.
    fn waiter_gets_it(system: &mut System, (holder, waiter, handle): (ThreadId, ThreadId, u32)) {
        assert_eq!(system.kernel.thread(holder).status, ThreadStatus::Dead);
        assert_eq!(mutex_state(system, handle), (None, 0));
        let tick = system.cpu.cycles;
        system.kernel.schedule(&mut system.cpu, tick);
        assert_eq!(system.kernel.current_thread, Some(waiter), "{}", system.kernel.describe_wait(waiter));
        assert!(matches!(system.kernel.thread(waiter).wait_result, Some(WaitResult::Signaled(0))));
        assert_eq!(mutex_state(system, handle), (Some(waiter), 1));
    }

    #[test]
    fn svc_exit_thread_lets_go_of_the_mutexes() {
        let mut system = System::new(Config::default());
        let threads = holder_and_waiter(&mut system);
        kernel::svc::dispatch(&mut system, 0x09);
        waiter_gets_it(&mut system, threads);
    }

    #[test]
    fn returning_from_the_entry_point_lets_go_of_the_mutexes() {
        let mut system = System::new(Config::default());
        let threads = holder_and_waiter(&mut system);
        system.cpu.regs[15] = THREAD_EXIT_MAGIC;
        assert_eq!(system.step(None), StepOutcome::Ran);
        waiter_gets_it(&mut system, threads);
    }

    #[test]
    fn a_branch_to_unmapped_memory_lets_go_of_the_mutexes() {
        let mut system = System::new(Config::default());
        let threads = holder_and_waiter(&mut system);
        system.cpu.regs[15] = 0x0000_1000;
        assert!(!system.memory.is_executable(0x0000_1000));
        assert_eq!(system.step(None), StepOutcome::Ran);
        waiter_gets_it(&mut system, threads);
    }

    /// one thread at the entry create_thread gives, its code a branch to
    /// itself, current and running.
    fn one_spinning_thread(system: &mut System) -> ThreadId {
        one_thread_running(system, &[0xEAFF_FFFE])
    }

    /// one thread running code at 0x00100000, current, with its stack and
    /// r0 in the same page, the stack at 0x00100F00 and r0 at 0x00100800.
    fn one_thread_running(system: &mut System, code: &[u32]) -> ThreadId {
        use memory::{MemoryState, Permission};
        let block = system.memory.phys.allocate(memory::MemoryRegion::Application, 0x1000).unwrap();
        system.memory.map(0x0010_0000, block.addr, 0x1000, Permission::RW | Permission::EXECUTE, MemoryState::Code);
        for (index, word) in code.iter().enumerate() {
            system.memory.write32(0x0010_0000 + index as u32 * 4, *word);
        }
        let id = system.kernel.create_thread("main", 0x0010_0000, 0x0010_0F00, 0, 0x30, 0);
        system.map_tls_page(id);
        system.kernel.schedule(&mut system.cpu, 0);
        assert_eq!(system.kernel.current_thread, Some(id));
        system.cpu.regs[0] = 0x0010_0800;
        id
    }

    /// runs steps until the run gets to tick or steps runs out, how many it
    /// took.
    fn steps_to(system: &mut System, tick: u64, steps: usize) -> usize {
        (0..steps).take_while(|_| {
            system.step(None);
            system.cpu.cycles < tick
        }).count() + 1
    }

    /// a thread alone on the core spinning on nothing waits for the next
    /// event, the next audio frame here, instead of the interpreter running
    /// every turn of its loop until then.
    #[test]
    fn a_thread_spinning_alone_skips_to_the_next_event() {
        let mut system = System::new(Config::default());
        one_spinning_thread(&mut system);
        let audio = system.next_audio_frame;
        assert!(steps_to(&mut system, audio, 40_000) < 20_000, "skipped");
        assert_eq!(system.cpu.cycles, audio, "to the audio frame, not past it");
        assert!(system.idle.skipped > 0);
    }

    /// a loop that counts in memory changes something every turn, and runs
    /// every one of them.
    #[test]
    fn a_loop_that_counts_is_not_skipped() {
        let mut system = System::new(Config::default());
        // ldr r1, [r0]; add r1, r1, #1; str r1, [r0]; b back
        one_thread_running(&mut system, &[0xE590_1000, 0xE281_1001, 0xE580_1000, 0xEAFF_FFFB]);
        for _ in 0..40_000 {
            system.step(None);
        }
        assert_eq!(system.idle.skipped, 0);
        assert_eq!(system.memory.read32(0x0010_0800), 10_000, "every turn counted");
    }

    /// a loop that puts back what it wrote, the way taking and letting go of
    /// a lock does, and pushes and pops, changes nothing and is skipped.
    #[test]
    fn a_loop_that_puts_back_what_it_wrote_is_skipped() {
        let mut system = System::new(Config::default());
        // ldr r1, [r0]; add r1, r1, #1; str r1, [r0]; sub r1, r1, #1;
        // str r1, [r0]; push {r4, lr}; pop {r4, lr}; b back
        let code = [0xE590_1000, 0xE281_1001, 0xE580_1000, 0xE241_1001, 0xE580_1000, 0xE92D_4010, 0xE8BD_4010, 0xEAFF_FFF7];
        one_thread_running(&mut system, &code);
        system.memory.write32(0x0010_0800, 7);
        let audio = system.next_audio_frame;
        assert!(steps_to(&mut system, audio, 40_000) < 20_000, "skipped");
        assert_eq!(system.memory.read32(0x0010_0800), 7);
    }

    /// a skip stops at the wake of a thread sleeping, which gets the core
    /// then.
    #[test]
    fn a_skip_stops_where_a_sleeping_thread_wakes() {
        let mut system = System::new(Config::default());
        one_spinning_thread(&mut system);
        let sleeper = system.kernel.create_thread("sleeper", 0x0010_0000, 0x0010_0E00, 0, 0x20, 0);
        system.map_tls_page(sleeper);
        let thread = system.kernel.thread_mut(sleeper);
        thread.status = ThreadStatus::Sleeping;
        thread.wakeup_at = Some(20_000);
        let steps = (0..40_000).take_while(|_| {
            system.step(None);
            system.kernel.current_thread != Some(sleeper)
        });
        assert!(steps.count() < 20_000, "skipped");
        assert!((20_000..20_008).contains(&system.cpu.cycles), "woke at {}", system.cpu.cycles);
    }

    /// waits on a handle for good, as svcWaitSynchronization1.
    fn wait_on(system: &mut System, handle: u32) {
        system.cpu.regs[0] = handle;
        system.cpu.regs[2] = u32::MAX;
        system.cpu.regs[3] = u32::MAX;
        kernel::svc::dispatch(system, 0x24);
    }

    /// steps until the thread runs again, the tick it does.
    fn runs_again(system: &mut System, id: ThreadId) -> u64 {
        for _ in 0..1000 {
            if system.kernel.current_thread == Some(id) && system.kernel.thread(id).status == ThreadStatus::Running {
                return system.cpu.cycles;
            }
            system.step(None);
        }
        panic!("the thread never ran again");
    }

    /// a wait the very next schedule ends, on a thread that keeps the core
    /// as nothing else can run, returns its result before the thread goes
    /// on, rather than the handle it was given, written over later.
    #[test]
    fn a_wait_the_next_schedule_ends_returns_its_result() {
        let mut system = System::new(Config::default());
        let id = one_spinning_thread(&mut system);
        system.cpu.regs[1] = 0;
        kernel::svc::dispatch(&mut system, 0x17);
        let event = system.cpu.regs[1];
        wait_on(&mut system, event);
        assert_eq!(system.kernel.thread(id).status, ThreadStatus::WaitSync);
        let object = system.kernel.resolve(event).unwrap();
        system.kernel.signal_event(object);
        system.step(None);
        assert_eq!(system.kernel.current_thread, Some(id));
        assert_eq!(system.cpu.regs[0], 0, "the wait's result, not the handle");
    }

    /// the top screen's second buffer is the right eye's only while its
    /// format asks for the 3D the way the console's GSP takes it, bit 5 set
    /// and bit 6 clear, and the title gave it one.
    #[test]
    fn the_right_eye_shows_only_while_the_title_asks_for_3d() {
        let mut system = System::new(Config::default());
        let config = &mut system.gpu.framebuffers[0];
        config.address_a_left = 0x1830_0000;
        config.address_a_right = 0x1834_6500;
        for (format, right) in [(0x20, true), (0x60, false), (0x40, false), (0x00, false)] {
            system.gpu.framebuffers[0].format = 0x0300 | format | 1;
            assert_eq!(system.draws_in_3d(), right, "format 0x{format:X}");
        }
        system.gpu.framebuffers[0].address_a_right = 0;
        system.gpu.framebuffers[0].format = 0x0321;
        assert!(!system.draws_in_3d(), "no buffer for the right eye");
        // the slider games read moves with it
        system.set_slider_3d(0.5);
        let page = system.memory.phys.shared_page_mut();
        assert_eq!((f32::from_le_bytes(page[0x80..0x84].try_into().unwrap()), page[0x84]), (0.5, 0), "the LED not forced off");
    }

    /// a zero sleep with nobody else ready to run is a no-op: the calling
    /// thread keeps the core, and no scheduling pass is due. titles poll a
    /// worker in a tight svcSleepThread(0) loop, and paying a full pass per
    /// iteration there was most of what one such title spent.
    #[test]
    fn a_zero_sleep_with_nobody_else_ready_does_not_schedule() {
        let mut system = System::new(Config::default());
        let id = one_spinning_thread(&mut system);
        system.kernel.reschedule_pending = false;
        system.cpu.regs[0] = 0;
        system.cpu.regs[1] = 0;
        kernel::svc::dispatch(&mut system, 0x0A);
        assert_eq!(system.kernel.thread(id).status, ThreadStatus::Running, "it keeps the core");
        assert!(!system.kernel.reschedule_pending, "no pass is due");
        assert_eq!(system.cpu.regs[0], 0);
    }

    /// a zero sleep with another thread ready still steps aside, so a title
    /// polling a worker keeps handing it the core.
    #[test]
    fn a_zero_sleep_with_another_ready_steps_aside() {
        let mut system = System::new(Config::default());
        let first = one_spinning_thread(&mut system);
        let second = system.kernel.create_thread("second", 0x0010_0000, 0x0FF0_0000, 0, 0x30, 0);
        system.map_tls_page(second);
        system.kernel.thread_mut(second).status = ThreadStatus::Ready;
        system.cpu.regs[0] = 0;
        system.cpu.regs[1] = 0;
        kernel::svc::dispatch(&mut system, 0x0A);
        assert_eq!(system.kernel.thread(first).status, ThreadStatus::Sleeping, "it steps aside");
        assert!(system.kernel.reschedule_pending, "the ready thread gets a pass");
    }

    /// a zero sleep while a thread is due to wake still has the pass then,
    /// so that thread wakes on time, not at the next preemption.
    #[test]
    fn a_zero_sleep_wakes_a_thread_that_is_due() {
        let mut system = System::new(Config::default());
        let first = one_spinning_thread(&mut system);
        let second = system.kernel.create_thread("second", 0x0010_0000, 0x0FF0_0000, 0, 0x30, 0);
        system.map_tls_page(second);
        let thread = system.kernel.thread_mut(second);
        thread.status = ThreadStatus::Sleeping;
        thread.wakeup_at = Some(system.cpu.cycles);
        system.kernel.reschedule_pending = false;
        system.cpu.regs[0] = 0;
        system.cpu.regs[1] = 0;
        kernel::svc::dispatch(&mut system, 0x0A);
        assert_eq!(system.kernel.thread(first).status, ThreadStatus::Sleeping, "it steps aside for the pass");
        assert!(system.kernel.reschedule_pending, "the sleeper due gets a pass");
    }

    /// a request to a service has the thread wait for the reply, and a
    /// thread below it runs meanwhile, as on the console. Star Fox 64 3D
    /// marks a save job running once the more urgent thread doing it is
    /// made, which has to be waiting on its first request by then.
    #[test]
    fn a_request_lets_the_threads_below_run_until_the_reply() {
        let mut system = System::new(Config::default());
        let id = one_spinning_thread(&mut system);
        let below = system.kernel.create_thread("below", 0x0010_0000, 0x0FF0_0000, 0, 0x31, 0);
        system.map_tls_page(below);
        let port = system.kernel.objects.insert(KObject::ClientPort("srv:".into()));
        let handle = system.kernel.handles.create(&mut system.kernel.objects, port, "srv:");
        // srv:RegisterClient
        let buffer = system.kernel.thread(id).tls + 0x80;
        system.memory.write32(buffer, 0x0001_0002);
        system.memory.write32(buffer + 4, 0x20);
        system.cpu.regs[0] = handle;
        let asked = system.cpu.cycles;
        kernel::svc::dispatch(&mut system, 0x32);
        assert_eq!((system.cpu.regs[0], system.memory.read32(buffer + 4)), (0, 0), "answered");
        assert_eq!(system.kernel.thread(id).status, ThreadStatus::Sleeping, "waiting for the reply");

        assert!(runs_again(&mut system, below) < asked + 100, "the thread below runs at once");
        let mut steps = 0;
        while system.kernel.current_thread != Some(id) || system.kernel.thread(id).status != ThreadStatus::Running {
            system.step(None);
            steps += 1;
            assert!(steps < 40_000, "the thread never got its reply");
        }
        let due = asked + kernel::svc::REQUEST_TICKS;
        assert!((due..due + PREEMPT_INTERVAL).contains(&system.cpu.cycles), "replied at {}, due at {due}", system.cpu.cycles);
    }

    /// a timer fires at its tick, not at the next frame, and the thread on
    /// it wakes then. a pulse timer is gone once it woke a waiter, and comes
    /// again a period after it was due.
    #[test]
    fn a_timer_fires_at_its_tick() {
        let mut system = System::new(Config::default());
        let id = one_spinning_thread(&mut system);
        // a pulse timer, due in a millisecond and every millisecond after
        system.cpu.regs[1] = 2;
        kernel::svc::dispatch(&mut system, 0x1A);
        let timer = system.cpu.regs[1];
        system.cpu.regs[0] = timer;
        system.cpu.regs[2] = 1_000_000;
        system.cpu.regs[3] = 0;
        system.cpu.regs[1] = 1_000_000;
        system.cpu.regs[4] = 0;
        kernel::svc::dispatch(&mut system, 0x1B);
        let period = kernel::thread::nanos_to_ticks(1_000_000);

        wait_on(&mut system, timer);
        let woke = runs_again(&mut system, id);
        assert!((period..period + PREEMPT_INTERVAL).contains(&woke), "woke at {woke}, due at {period}");
        assert_eq!(system.cpu.regs[0], 0);

        wait_on(&mut system, timer);
        assert_eq!(system.kernel.thread(id).status, ThreadStatus::WaitSync, "the pulse is gone");
        let woke = runs_again(&mut system, id);
        assert!((2 * period..2 * period + PREEMPT_INTERVAL).contains(&woke), "woke at {woke}, due at {}", 2 * period);
    }

    /// a pulse timer due in a millisecond and every millisecond after.
    fn pulse_timer(system: &mut System) -> u32 {
        system.cpu.regs[1] = 2;
        kernel::svc::dispatch(system, 0x1A);
        let timer = system.cpu.regs[1];
        system.cpu.regs[0] = timer;
        system.cpu.regs[2] = 1_000_000;
        system.cpu.regs[3] = 0;
        system.cpu.regs[1] = 1_000_000;
        system.cpu.regs[4] = 0;
        kernel::svc::dispatch(system, 0x1B);
        timer
    }

    /// a pulse timer releases every thread waiting on it when it fires, not
    /// only the first, and a fire nothing waits for is lost.
    #[test]
    fn a_pulse_timer_wakes_every_waiter_then_clears() {
        let mut system = System::new(Config::default());
        let first = one_spinning_thread(&mut system);
        let second = system.kernel.create_thread("second", 0x0010_0000, 0x0FF0_0000, 0, 0x30, 0);
        system.map_tls_page(second);
        let timer = pulse_timer(&mut system);
        let period = kernel::thread::nanos_to_ticks(1_000_000);
        for id in [first, second] {
            system.kernel.current_thread = Some(id);
            system.kernel.thread_mut(id).status = ThreadStatus::Running;
            wait_on(&mut system, timer);
            assert_eq!(system.kernel.thread(id).status, ThreadStatus::WaitSync);
        }
        system.kernel.current_thread = None;
        while system.cpu.cycles < period {
            system.step(None);
        }
        system.step(None);
        for id in [first, second] {
            assert_ne!(system.kernel.thread(id).status, ThreadStatus::WaitSync, "thread {id} woke");
        }

        // the next fire comes with nothing waiting, and is gone after
        system.cpu.cycles = 2 * period + 10;
        system.kernel.current_thread = Some(first);
        system.kernel.thread_mut(first).status = ThreadStatus::Running;
        wait_on(&mut system, timer);
        assert_eq!(system.kernel.thread(first).status, ThreadStatus::WaitSync, "the fire nothing waited for is lost");
    }

    /// cancelling a timer that came due before the scheduler looked keeps
    /// its fire, as the console's had already happened.
    #[test]
    fn cancelling_a_timer_due_keeps_its_fire() {
        let mut system = System::new(Config::default());
        let id = one_spinning_thread(&mut system);
        system.cpu.regs[1] = 0;
        kernel::svc::dispatch(&mut system, 0x1A);
        let timer = system.cpu.regs[1];
        system.cpu.regs[0] = timer;
        system.cpu.regs[2] = 1_000_000;
        system.cpu.regs[3] = 0;
        system.cpu.regs[1] = 0;
        system.cpu.regs[4] = 0;
        kernel::svc::dispatch(&mut system, 0x1B);
        let other = system.kernel.create_thread("other", 0x0010_0000, 0x0FF0_0000, 0, 0x30, 0);
        system.map_tls_page(other);
        wait_on(&mut system, timer);
        // the other thread cancels it after it came due
        system.cpu.cycles = kernel::thread::nanos_to_ticks(1_000_000) + 100;
        system.kernel.current_thread = Some(other);
        system.kernel.thread_mut(other).status = ThreadStatus::Running;
        system.cpu.regs[0] = timer;
        kernel::svc::dispatch(&mut system, 0x1C);
        let tick = system.cpu.cycles;
        system.kernel.schedule(&mut system.cpu, tick);
        assert_ne!(system.kernel.thread(id).status, ThreadStatus::WaitSync, "the waiter woke");
    }

    /// a handle closed while a thread waits on its object neither wakes the
    /// thread as if it was signalled nor gives the object's place to the
    /// next one made, and the object goes once the wait is over.
    #[test]
    fn closing_what_a_thread_waits_on_keeps_it_waiting() {
        let mut system = System::new(Config::default());
        let id = one_spinning_thread(&mut system);
        system.cpu.regs[1] = 0;
        kernel::svc::dispatch(&mut system, 0x17);
        let event = system.cpu.regs[1];
        let object = system.kernel.resolve(event).unwrap();
        // for a millisecond
        system.cpu.regs[0] = event;
        system.cpu.regs[2] = 1_000_000;
        system.cpu.regs[3] = 0;
        kernel::svc::dispatch(&mut system, 0x24);
        let other = system.kernel.create_thread("other", 0x0010_0000, 0x0FF0_0000, 0, 0x30, 0);
        system.map_tls_page(other);
        system.kernel.current_thread = Some(other);
        system.kernel.thread_mut(other).status = ThreadStatus::Running;
        system.cpu.regs[0] = event;
        kernel::svc::dispatch(&mut system, 0x23);
        let tick = system.cpu.cycles;
        system.kernel.schedule(&mut system.cpu, tick);
        assert_eq!(system.kernel.thread(id).status, ThreadStatus::WaitSync, "not woken as if signalled");
        system.cpu.regs[1] = 0;
        kernel::svc::dispatch(&mut system, 0x17);
        assert_ne!(system.kernel.resolve(system.cpu.regs[1]), Some(object));

        let late = kernel::thread::nanos_to_ticks(1_000_000) + 1;
        system.kernel.schedule(&mut system.cpu, late);
        assert!(matches!(system.kernel.thread(id).wait_result, Some(kernel::thread::WaitResult::TimedOut)));
        assert!(system.kernel.objects.get(object).is_none(), "gone after the wait");
    }

    /// a pulse event wakes what waits on it when signalled and is gone
    /// after, the next wait on it waits.
    #[test]
    fn a_pulse_event_does_not_stay_signalled() {
        let mut system = System::new(Config::default());
        let id = one_spinning_thread(&mut system);
        system.cpu.regs[1] = 2;
        kernel::svc::dispatch(&mut system, 0x17);
        let event = system.cpu.regs[1];
        wait_on(&mut system, event);
        let object = system.kernel.resolve(event).unwrap();
        system.kernel.signal_event(object);
        runs_again(&mut system, id);
        wait_on(&mut system, event);
        assert_eq!(system.kernel.thread(id).status, ThreadStatus::WaitSync);
    }

    /// an arbiter wait's timeout takes r5 as its high word, -1 waits for
    /// good.
    #[test]
    fn an_arbiter_timeout_is_64_bits() {
        let mut system = System::new(Config::default());
        let id = one_spinning_thread(&mut system);
        kernel::svc::dispatch(&mut system, 0x21);
        let arbiter = system.cpu.regs[1];
        let address = system.kernel.thread(id).tls + 0x100;
        system.memory.write32(address, 0);
        for (low, high, wakes) in [(u32::MAX, u32::MAX, false), (0x2A05_F200, 1, true)] {
            system.kernel.thread_mut(id).status = ThreadStatus::Running;
            system.cpu.regs[0] = arbiter;
            system.cpu.regs[1] = address;
            // wait if less than, with a timeout
            system.cpu.regs[2] = 3;
            system.cpu.regs[3] = 1;
            system.cpu.regs[4] = low;
            system.cpu.regs[5] = high;
            kernel::svc::dispatch(&mut system, 0x22);
            let wakeup = system.kernel.thread(id).wakeup_at;
            assert_eq!(wakeup.is_some(), wakes);
            if wakes {
                // five seconds
                let five = kernel::thread::nanos_to_ticks(5_000_000_000);
                assert!(wakeup.unwrap() >= five, "{wakeup:?}");
            }
        }
    }

    /// a thread whose timed arbiter wait ran out, and that then ended, left
    /// its entry on the arbiter. a signal for one waiter on the address
    /// passes over it to the thread really waiting there, and the dead one
    /// stays dead.
    #[test]
    fn an_arbiter_signal_wakes_only_threads_still_waiting() {
        let mut system = System::new(Config::default());
        let ended = system.kernel.create_thread("ended", 0x0010_0000, 0x1000_0000, 0, 0x30, 0);
        let signaller = system.kernel.create_thread("signaller", 0x0010_0000, 0x0FF0_0000, 0, 0x30, 0);
        let parked = system.kernel.create_thread("parked", 0x0010_0000, 0x0FE0_0000, 0, 0x30, 0);
        for id in [ended, signaller, parked] {
            system.map_tls_page(id);
        }
        system.kernel.schedule(&mut system.cpu, 0);
        assert_eq!(system.kernel.current_thread, Some(ended));
        kernel::svc::dispatch(&mut system, 0x21);
        let arbiter = system.cpu.regs[1];
        let address = system.kernel.thread(ended).tls + 0x100;
        system.memory.write32(address, 0);
        let arbitrate = |system: &mut System, who: ThreadId, kind: u32| {
            system.kernel.current_thread = Some(who);
            system.kernel.thread_mut(who).status = ThreadStatus::Running;
            system.cpu.regs[0] = arbiter;
            system.cpu.regs[1] = address;
            system.cpu.regs[2] = kind;
            system.cpu.regs[3] = 1;
            system.cpu.regs[4] = 1000;
            kernel::svc::dispatch(system, 0x22);
        };
        // waits if less than 1, for a microsecond, and times out
        arbitrate(&mut system, ended, 3);
        system.kernel.current_thread = None;
        system.kernel.schedule(&mut system.cpu, 1_000_000);
        assert!(matches!(system.kernel.thread(ended).wait_result, Some(WaitResult::TimedOut)));
        // then ends
        system.kernel.current_thread = Some(ended);
        system.kernel.thread_mut(ended).status = ThreadStatus::Running;
        kernel::svc::dispatch(&mut system, 0x09);
        // another waits for good, and a third signals one waiter
        arbitrate(&mut system, parked, 1);
        assert_eq!(system.kernel.thread(parked).status, ThreadStatus::WaitArbiter);
        arbitrate(&mut system, signaller, 0);
        assert_eq!(system.kernel.thread(ended).status, ThreadStatus::Dead);
        assert_eq!(system.kernel.thread(parked).status, ThreadStatus::Ready);
    }

    /// a title drawing every other refresh alternates its own pictures with
    /// the ones in between at the host's times, a full-rate title shows its
    /// own, a stalled one shows its own, and host times outside the two
    /// pictures clamp to them.
    #[test]
    fn pictures_in_between_are_shown_at_host_times() {
        use std::time::Duration;

        // two console refreshes between the title's pictures, about 33.43 ms.
        let frame_time = Duration::from_secs_f64(2.0 / 59.83);
        let half = frame_time / 2;
        let mut pacing = Pacing {
            refreshes: 10,
            // the last new picture arrived one refresh ago
            new_at: 9,
            between: true,
            older_host_time: Some(Duration::ZERO),
            newer_host_time: Some(frame_time),
            ..Default::default()
        };

        // at the older picture's time, the older picture shows, the frame
        // before, rather than jumping to the newest one early
        assert_eq!(pacing.kind_at_host_time(Duration::ZERO), PictureKind::InBetween(0.0));
        // halfway between the two, the picture in between at t=0.5 shows
        match pacing.kind_at_host_time(half) {
            PictureKind::InBetween(t) => assert!((t - 0.5).abs() < 1e-5, "t was {t}"),
            kind => panic!("expected InBetween(0.5), got {kind:?}"),
        }
        // at the newer picture's time, the title's own shows again
        assert_eq!(pacing.kind_at_host_time(frame_time), PictureKind::Real);
        // beyond it, the title's own stays until a newer picture arrives
        assert_eq!(pacing.kind_at_host_time(frame_time + half), PictureKind::Real);

        // a title drawing every refresh has no picture in between at 60 Hz,
        // but a 120 Hz host asks for one halfway between its frames
        let frame_time = Duration::from_secs_f64(1.0 / 59.83);
        pacing.older_host_time = Some(Duration::ZERO);
        pacing.newer_host_time = Some(frame_time);
        pacing.between = false;
        match pacing.kind_at_host_time(frame_time / 2) {
            PictureKind::InBetween(t) => assert!((t - 0.5).abs() < 1e-5, "t was {t}"),
            kind => panic!("expected InBetween(0.5), got {kind:?}"),
        }

        // a title stalled more than two refreshes without a new picture
        // shows its own, whatever the host time
        pacing.between = true;
        pacing.new_at = 7;
        assert_eq!(pacing.kind_at_host_time(frame_time / 2), PictureKind::Real);

        // host times before the two pictures clamp to the older one, and
        // after them to the newest
        pacing.new_at = 9;
        assert_eq!(pacing.kind_at_host_time(Duration::from_millis(0)), PictureKind::InBetween(0.0));
        assert_eq!(pacing.kind_at_host_time(frame_time), PictureKind::Real);
        assert_eq!(pacing.kind_at_host_time(frame_time + Duration::from_millis(1)), PictureKind::Real);
    }

    /// a Screen capped at 60 shows the title's own pictures, never one in
    /// between, and the cap is per Screen: another Screen's leaves the
    /// shown one drawing in between.
    #[test]
    fn a_screen_capped_at_sixty_shows_real_frames_only() {
        let (mut system, frame) = stereo_system();
        let half = frame / 2;
        system.aim_mouse = Some(aim_mouse::AimMouse::default());
        assert!(matches!(system.picture_kind_at_host_time(half), PictureKind::InBetween(_)), "uncapped, the host sees a picture in between");
        assert!(system.shows_between());

        system.aim_mouse.as_mut().unwrap().set_screen_rate(aim_mouse::Screen::Pause, aim_mouse::ScreenRate::Sixty);
        system.aim_mouse.as_mut().unwrap().show_screen(aim_mouse::Screen::Pause, system.frames);
        assert_eq!(system.picture_kind_at_host_time(half), PictureKind::Real);
        assert!(!system.shows_between(), "a capped Screen draws no picture in between");

        // another Screen's cap does not reach Pause
        system.aim_mouse.as_mut().unwrap().set_screen_rate(aim_mouse::Screen::Pause, aim_mouse::ScreenRate::Host);
        system.aim_mouse.as_mut().unwrap().set_screen_rate(aim_mouse::Screen::MainMenu, aim_mouse::ScreenRate::Sixty);
        system.aim_mouse.as_mut().unwrap().show_screen(aim_mouse::Screen::Pause, system.frames);
        assert!(matches!(system.picture_kind_at_host_time(half), PictureKind::InBetween(_)));

        // until the capped Screen is the one shown
        system.aim_mouse.as_mut().unwrap().show_screen(aim_mouse::Screen::MainMenu, system.frames);
        assert_eq!(system.picture_kind_at_host_time(half), PictureKind::Real);
        assert!(!system.shows_between());
    }

    /// a 144 Hz host, not a multiple of the game's 60, still shows the
    /// pictures in between at the fraction of the frame the clock is at:
    /// t = 0.417 and 0.833, every one within [0, 1].
    #[test]
    fn pictures_in_between_follow_a_144_hz_host_clock() {
        use std::time::Duration;

        let frame_time = Duration::from_secs_f64(1.0 / 60.0);
        let host_step = Duration::from_secs_f64(1.0 / 144.0);
        let pacing = Pacing {
            refreshes: 10,
            // the last new picture arrived one refresh ago
            new_at: 9,
            between: true,
            older_host_time: Some(Duration::ZERO),
            newer_host_time: Some(frame_time),
            ..Default::default()
        };

        // the refreshes inside one frame: the older picture at t=0, then
        // two in between it and the next
        let mut shown = Vec::new();
        let mut host = Duration::ZERO;
        while host < frame_time {
            shown.push(pacing.kind_at_host_time(host));
            host += host_step;
        }
        assert_eq!(shown.first(), Some(&PictureKind::InBetween(0.0)), "{shown:?}");
        let between: Vec<f32> = shown.iter().filter_map(PictureKind::t).collect();
        assert_eq!(between.len(), 3, "{shown:?}");
        for (t, expected) in between.iter().zip([0.0, 60.0 / 144.0, 120.0 / 144.0]) {
            assert!((t - expected).abs() < 1e-4, "t was {t}, expected {expected}");
            assert!((0.0..=1.0).contains(t), "t was {t}");
        }
        // the next frame's own picture starts the sequence over
        assert_eq!(pacing.kind_at_host_time(frame_time), PictureKind::Real);
    }

    /// the top screen as a title drawing in 3D has it, with a buffer for the
    /// right eye, and the pacer as pace leaves it for a title drawing every
    /// other refresh, two console refreshes apart.
    fn stereo_system() -> (System, std::time::Duration) {
        use std::time::Duration;

        let mut system = System::new(Config::default());
        // bit 5 set and bit 6 clear ask for the screen's two buffers
        system.gpu.framebuffers[0].format = 0x20;
        system.gpu.framebuffers[0].address_a_right = 0x1834_6500;
        assert!(system.draws_in_3d(), "the top screen has a buffer for each eye");
        let frame = Duration::from_secs_f64(2.0 / 59.83);
        system.pacing.on = true;
        system.pacing.refreshes = 10;
        system.pacing.new_at = 9;
        system.pacing.between = true;
        system.pacing.older_host_time = Some(Duration::ZERO);
        system.pacing.newer_host_time = Some(frame);
        (system, frame)
    }

    /// with 3D on, both eyes show the same kind of picture at the same t for
    /// every host time, as the pacer decides it once for the pair: a host
    /// at 120 Hz alternates the title's own pictures with the ones in
    /// between at t = ½, and neither eye is ever at a t the other is not.
    #[test]
    fn both_eyes_show_the_same_picture_in_between_at_host_times() {
        use std::time::Duration;

        let (mut system, frame) = stereo_system();
        let step = Duration::from_secs_f64(1.0 / 120.0);
        let (mut own, mut between) = (0, 0);
        let mut host = Duration::ZERO;
        while host <= frame {
            let pair = system.stereo_picture_at_host_time(host);
            let right = pair.right.as_ref().expect("the title draws in 3D");
            assert_eq!(pair.left.kind, right.kind, "the eyes' kind at {host:?}");
            assert_eq!(pair.left.kind.t(), right.kind.t(), "the eyes' t at {host:?}");

            // the presenter's queries at the same host time agree with the
            // pair, one kind for both eyes
            let gpu = system.gpu_stereo_picture_at_host_time(host);
            let (_, left_kind) = system.gpu_picture_at_host_time(zakuro_common::Screen::Top, false, host);
            let (_, right_kind) = system.gpu_picture_at_host_time(zakuro_common::Screen::Top, true, host);
            assert_eq!(left_kind, right_kind, "the eyes' kinds at {host:?}");
            assert_eq!(gpu.kind, left_kind);
            assert_eq!(pair.left.kind, left_kind);

            match pair.left.kind {
                PictureKind::Real => own += 1,
                PictureKind::InBetween(t) => {
                    assert!((0.0..1.0).contains(&t), "t was {t} at {host:?}");
                    between += 1;
                }
            }
            if host >= frame {
                break;
            }
            // the last sample lands on the frame's own time, its own picture
            host = (host + step).min(frame);
        }
        assert!(own > 0 && between > 0, "{own} of the title's own pictures and {between} in between");
    }

    /// the eyes never part as the frames and the host's refreshes advance: a
    /// host at 144 Hz asks for a new t each refresh, and every time both
    /// eyes are given the same kind and t.
    #[test]
    fn the_eyes_never_part_as_the_host_times_advance() {
        use std::time::Duration;

        let (mut system, _) = stereo_system();
        let frame = Duration::from_secs_f64(1.0 / 59.83);
        let step = Duration::from_secs_f64(1.0 / 144.0);
        let mut older = Duration::ZERO;
        let mut between = 0;
        for _ in 0..8 {
            let newer = older + frame;
            system.pacing.refreshes += 1;
            system.pacing.new_at = system.pacing.refreshes;
            system.pacing.older_host_time = Some(older);
            system.pacing.newer_host_time = Some(newer);
            let mut host = older;
            while host < newer {
                let pair = system.gpu_stereo_picture_at_host_time(host);
                let (_, left_kind) = system.gpu_picture_at_host_time(zakuro_common::Screen::Top, false, host);
                let (_, right_kind) = system.gpu_picture_at_host_time(zakuro_common::Screen::Top, true, host);
                assert_eq!(left_kind, right_kind, "the eyes' kinds at {host:?}");
                assert_eq!(pair.kind, left_kind);
                if let Some(t) = left_kind.t() {
                    assert!((0.0..1.0).contains(&t), "t was {t} at {host:?}");
                    between += 1;
                }
                host += step;
            }
            older = newer;
        }
        assert!(between > 0, "144 Hz asks for pictures in between");
    }

    /// the right eye's picture comes from its own buffer, not the left's, so
    /// a stereo picture shows both eyes as the title drew them.
    #[test]
    fn each_eye_shows_its_own_picture() {
        use memory::{MemoryState, Permission};

        let mut system = System::new(Config::default());
        // a top screen's buffer for each eye, each filled with its own pixel
        let size = zakuro_common::Screen::Top.width() * zakuro_common::Screen::Top.height() * 4;
        let mut buffer = |at: u32, pixel: [u8; 4]| {
            let block = system.memory.phys.allocate(memory::MemoryRegion::Application, size).unwrap();
            system.memory.map(at, block.addr, size, Permission::RW, MemoryState::Private);
            let mut bytes = vec![0u8; size as usize];
            for chunk in bytes.chunks_exact_mut(4) {
                chunk.copy_from_slice(&pixel);
            }
            system.memory.write_bytes(at, &bytes);
            at
        };
        let left = buffer(0x0100_0000, [1, 2, 3, 4]);
        let right = buffer(0x0200_0000, [5, 6, 7, 8]);
        let config = &mut system.gpu.framebuffers[0];
        config.format = 0x20;
        config.address_a_left = left;
        config.address_a_right = right;
        assert!(system.draws_in_3d());

        let pair = system.stereo_picture_at_host_time(std::time::Duration::ZERO);
        let right = pair.right.expect("the title draws in 3D");
        assert_eq!(pair.left.kind, right.kind);
        assert_ne!(pair.left.screen.0[..4], right.screen.0[..4], "each eye reads its own buffer");
    }

    /// a title not drawing in 3D has no right eye to show, and the pair is
    /// the left eye alone.
    #[test]
    fn without_3d_there_is_no_right_eye_picture() {
        let mut system = System::new(Config::default());
        assert!(!system.draws_in_3d());
        let pair = system.stereo_picture_at_host_time(std::time::Duration::ZERO);
        assert!(pair.right.is_none());
        let gpu = system.gpu_stereo_picture_at_host_time(std::time::Duration::ZERO);
        assert!(gpu.right.is_none());
        assert_eq!(gpu.kind, pair.left.kind);
    }

    /// switching High Refresh Mode on or off starts the pacer over, so no
    /// picture in between is timed from before the switch.
    #[test]
    fn switching_the_mode_starts_the_pacer_over() {
        let mut system = System::new(Config::default());
        for on in [true, false] {
            system.pacing.refreshes = 12;
            system.pacing.new_at = 7;
            system.pacing.between = true;
            system.pacing.older_host_time = Some(std::time::Duration::ZERO);
            system.pacing.newer_host_time = Some(std::time::Duration::from_millis(16));
            system.set_interpolation(on);
            assert_eq!(system.pacing.refreshes, 0, "the pacer starts over, on={on}");
            assert_eq!(system.pacing.new_at, 0, "the pacer starts over, on={on}");
            assert!(!system.pacing.between, "no picture in between is kept, on={on}");
            assert!(system.pacing.older_host_time.is_none(), "no frame times are kept, on={on}");
            assert!(system.pacing.newer_host_time.is_none(), "no frame times are kept, on={on}");
        }
    }
}
