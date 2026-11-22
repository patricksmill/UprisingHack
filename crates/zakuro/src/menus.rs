//! the library, the menu over a game and the settings, drawn w/ egui

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use egui::{Align, Color32, Layout, RichText, Vec2};
use winit::keyboard::KeyCode;
use zakuro_core::aim_mouse::{CauldronRules, DialogRules, LoadingRules, NavigationDirection, PauseRules, PlacementRule, RuleScale};
use zakuro_core::services::keyboard::Request;

use crate::library::{Game, Library, ICON_SIZE};
use crate::recompile::{Job, Stage, Wanted};
use crate::gamepad::button_name;
use crate::settings::{
    game_key, CameraSource, Filter, GameSettings, Keys, Language, MicrophoneSource, Orientation, PadButtons, Renderer, Screens, Settings, TopShape,
};

/// the rules of the Screen the layout editor edits, which pick its window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditorRules {
    Pause(PauseRules),
    RetreatDialog(DialogRules),
    Cauldron(CauldronRules),
    Loading(LoadingRules),
}

/// what the user asked for
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Play(PathBuf),
    /// recompile these games of the library, one after another
    Recompile(Vec<Wanted>),
    /// download a compiler, then recompile these games of the library w/ it
    DownloadCompiler(Vec<Wanted>),
    /// show the mods folder of the game w/ this program id
    Mods(u64),
    /// stop recompiling the game w/ this id, or waiting to
    CancelRecompile(u64),
    /// leave only the game recompiling now, none of those waiting
    CancelWaiting,
    ChooseFolder,
    ChooseBackground,
    /// pick the picture the cameras see
    ChooseCameraPicture,
    /// save the game's state to the chosen slot, or load it from there
    SaveState,
    LoadState,
    /// open a room and join it, join someone else's, or leave the one joined
    HostRoom,
    JoinRoom,
    LeaveRoom,
    /// take part in the Download Play offer at this place in the list, or leave the one taken part in
    DlpJoin(usize),
    DlpLeave,
    Rescan,
    Resume,
    Reset,
    Library,
    Fullscreen,
    /// copy what a report on the game needs
    CopyReport,
    Quit,
    /// the settings changed, to save and apply
    Settings,
    /// close the game's keyboard w/ this text and the button pressed
    Keyboard(String, usize),
    /// turn the running game's cheat at this index on or off
    Cheat(usize, bool),
    /// add a cheat, its name and its code, to the running game
    AddCheat(String, String),
    /// read the running game's cheats from their file again
    ReloadCheats,
    /// show the cheats folder (no judgment)
    CheatsFolder,
    /// the pause layout rules as the editor left them.
    PauseRules(PauseRules),
    /// the Retreat Dialog's rules, edited.
    DialogRules(DialogRules),
    /// the Fiend's Cauldron's rules, edited.
    CauldronRules(CauldronRules),
    /// the Midstage Loading's rules, edited.
    LoadingRules(LoadingRules),
    /// a Scripted Menu layer moved by the editor.
    EditorLayer(usize, [f32; 4]),
    /// a Scripted Menu layer hidden or shown again by the editor.
    EditorLayerVisible(usize, bool),
    /// an explicit navigation link set for a Control, or removed for the
    /// game's native behavior.
    EditorNavigationLink(usize, NavigationDirection, Option<usize>),
    /// keep the dev editor's Scripted Menu moves, hidden layers and
    /// navigation links.
    SaveLayout,
}

/// a named group of one Screen's editor layers. Linked, picking any member
/// picks them all, so they move, scale and nudge together.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EditorFolder {
    pub screen: usize,
    pub name: String,
    /// layer indices, as the editor lists them.
    pub parts: Vec<usize>,
    #[serde(default)]
    pub linked: bool,
    /// its members are left out of the list.
    #[serde(default)]
    pub collapsed: bool,
}

/// the edited Screen as it was before one edit: every layer's box and
/// visibility, and its folders.
#[derive(Clone, Debug, PartialEq)]
struct EditorSnapshot {
    layers: Vec<[f32; 4]>,
    visibility: Vec<bool>,
    folders: Vec<EditorFolder>,
}

/// at most this many edits are kept to undo.
const UNDO_DEPTH: usize = 200;

/// the group with every linked folder that shares a member filled in.
fn linked_group(group: &[usize], folders: &[EditorFolder]) -> Vec<usize> {
    let mut whole = group.to_vec();
    for folder in folders.iter().filter(|folder| folder.linked && folder.parts.iter().any(|part| group.contains(part))) {
        for &part in &folder.parts {
            if !whole.contains(&part) { whole.push(part); }
        }
    }
    whole
}

/// what the layer list asked for besides layer actions.
#[derive(Default)]
struct LayerListResponse {
    /// the Visible checkboxes, for the interaction tests.
    boxes: Vec<egui::Response>,
    /// a layer was shown or hidden: one edit to undo.
    edited: bool,
    undo: bool,
    redo: bool,
}

/// what the frame rate over the game reads: the game's own frames a second,
/// or, in High Refresh Mode, the host's pictures and the game's frames
/// apart so dropped pictures and dropped frames can be told apart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FpsReadout {
    /// the game's frames a second.
    Game(f32),
    /// pictures the host showed a second and frames the game ran.
    Split { shown: f32, game: f32 },
}

impl FpsReadout {
    /// the readout for a mode: High Refresh Mode shows the host's pictures
    /// and the game's frames apart.
    pub fn new(high_refresh: bool, shown: f32, game: f32) -> FpsReadout {
        if high_refresh { FpsReadout::Split { shown, game } } else { FpsReadout::Game(game) }
    }

    /// the line the overlay shows.
    pub fn text(self) -> String {
        match self {
            FpsReadout::Game(fps) => format!("{fps:.0} fps"),
            FpsReadout::Split { shown, game } => format!("{shown:.0} shown / {game:.0} game"),
        }
    }
}

#[derive(Default)]
pub struct Menus {
    /// the menu over a running game
    pub menu_open: bool,
    /// the developer layout editor over the game.
    pub editor_open: bool,
    /// the selected Scripted Menu layer and whether its corner is dragged.
    pub editor_layer: Option<usize>,
    /// every selected layer, the primary one included; Ctrl or Shift adds.
    editor_group: Vec<usize>,
    editor_resizing: bool,
    /// the boxes the dragged layers started at, and the layer names the
    /// selection was picked from, so another screen forgets both.
    editor_drag: Vec<(usize, [f32; 4])>,
    editor_seen: Vec<String>,
    /// the Scripted Menu the editor's layers belong to; folders are kept
    /// by it, so the Cauldron and Pause have none.
    pub editor_screen: Option<usize>,
    /// every Screen's layer folders, saved with the layout.
    pub editor_folders: Vec<EditorFolder>,
    /// the edits to step back through and forward again, the latest last.
    editor_undo: Vec<EditorSnapshot>,
    editor_redo: Vec<EditorSnapshot>,
    /// the folder whose name is being written, by its place on the Screen.
    editor_renaming: Option<usize>,
    /// the cheats window, and the cheat being written in it.
    pub cheats_open: bool,
    cheat_name: String,
    cheat_code: String,
    pub settings_open: bool,
    /// the UprisingHack window: Kid Icarus's PC Preferences.
    pub uprising_open: bool,
    /// which tab of the UprisingHack window is shown.
    uprising_tab: UprisingTab,
    /// the game whose own settings are open, its id and its name
    pub game_settings: Option<(u64, String)>,
    /// the binding waiting for a key, by its place in Keys::all_mut
    pub rebinding: Option<usize>,
    /// the controller binding waiting for a button, by its place in PadButtons::all_mut
    pub rebinding_pad: Option<usize>,
    /// Kid Icarus's Action Binding waiting for an input: the action, the
    /// device and the slot, 0 primary or 1 secondary.
    pub rebinding_action: Option<(crate::bindings::Action, crate::bindings::Device, usize)>,
    /// an input another action already uses, waiting for Swap, Replace or
    /// Cancel.
    pub binding_conflict: Option<(crate::bindings::Action, usize, crate::bindings::Source, Vec<crate::bindings::Action>)>,
    /// asks for the controller's gyroscope to be calibrated.
    pub calibrate_gyro: bool,
    /// what the controller's gyroscope can do, to show.
    pub gyro_status: String,
    /// a Kid Icarus category waiting for Restore Defaults to be confirmed.
    pending_reset: Option<crate::preferences::Category>,
    /// something to tell the user, until they close it.
    pub message: Option<String>,
    /// something to tell the user for a moment, over the game, and since when
    pub toast: Option<(String, std::time::Instant)>,
    /// the slot states get saved to and loaded from, 1 to 9
    pub slot: u8,
    /// the multiplayer window, and the password typed in it
    pub multiplayer_open: bool,
    pub room_password: String,
    /// the icons of the games offered for Download Play
    dlp_icons: HashMap<u64, egui::TextureHandle>,
    /// a report on the game, to copy from the message it goes w/, and from no other
    pub report: Option<(String, String)>,
    /// what's being typed into the game's keyboard, and what it asked for
    typing: Option<(Request, String)>,
    /// the library's games to recompile once the player agrees to download a compiler for them
    pub compiler_offer: Option<Vec<Wanted>>,
    icons: HashMap<PathBuf, egui::TextureHandle>,
    background: Background,
}

/// the room the player is in, as the multiplayer window shows it
pub struct RoomView {
    pub name: String,
    /// each member's nickname and the game they play
    pub members: Vec<(String, String)>,
    /// the port of the room this program holds open, when it's its own
    pub port: Option<u16>,
    /// Download Play, while no game runs
    pub download_play: Option<DlpView>,
}

/// the Download Play offers in the room, and where the guest is
pub struct DlpView {
    pub offers: Vec<OfferView>,
    /// what the guest is doing, and how far along, while it takes part
    pub doing: Option<(String, Option<f32>)>,
}

pub struct OfferView {
    pub child: u64,
    pub title: String,
    pub host: String,
    /// RGBA, ICON_SIZE square
    pub icon: Option<Vec<u8>>,
    pub guests: u8,
    pub most: u8,
    /// why the player can't take part, when they can't
    pub blocked: Option<String>,
}

/// the picture behind the library, decoded away from the UI
#[derive(Default)]
struct Background {
    /// the file shown or being read
    path: Option<PathBuf>,
    texture: Option<egui::TextureHandle>,
    loading: Option<Receiver<Result<egui::ColorImage, String>>>,
}

/// the longest side a background gets kept at, bigger ones get scaled down
const BACKGROUND_SIDE: u32 = 2560;

impl Background {
    /// the texture for path, starting to read it when it changed, none till it's ready or when there's no picture
    fn get(&mut self, ctx: &egui::Context, path: Option<&Path>) -> Result<Option<&egui::TextureHandle>, String> {
        if self.path.as_deref() != path {
            self.path = path.map(Path::to_owned);
            self.texture = None;
            self.loading = path.map(|path| {
                let (send, receive) = channel();
                let path = path.to_owned();
                std::thread::spawn(move || {
                    let _ = send.send(decode(&path));
                });
                receive
            });
        }
        if let Some(receive) = &self.loading {
            match receive.try_recv() {
                Ok(image) => {
                    self.loading = None;
                    self.texture = Some(ctx.load_texture("library background", image?, egui::TextureOptions::LINEAR));
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint(),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.loading = None,
            }
        }
        Ok(self.texture.as_ref())
    }
}

fn decode(path: &Path) -> Result<egui::ColorImage, String> {
    let image = image::open(path).map_err(|error| format!("could not open {}, {error}", path.display()))?;
    let image = if image.width().max(image.height()) > BACKGROUND_SIDE {
        image.resize(BACKGROUND_SIDE, BACKGROUND_SIDE, image::imageops::FilterType::Triangle)
    } else {
        image
    };
    let rgba = image.to_rgba8();
    Ok(egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw()))
}

/// the part of a picture that fills an area w/o stretching it, cutting off what sticks out on either side
fn cover(area: Vec2, picture: [usize; 2]) -> egui::Rect {
    let (width, height) = (picture[0].max(1) as f32, picture[1].max(1) as f32);
    let area_ratio = area.x / area.y.max(1.0);
    let picture_ratio = width / height;
    if area_ratio > picture_ratio {
        // wider area, the picture's top and bottom are cut
        let shown = picture_ratio / area_ratio;
        egui::Rect::from_min_max(egui::pos2(0.0, (1.0 - shown) / 2.0), egui::pos2(1.0, (1.0 + shown) / 2.0))
    } else {
        let shown = area_ratio / picture_ratio;
        egui::Rect::from_min_max(egui::pos2((1.0 - shown) / 2.0, 0.0), egui::pos2((1.0 + shown) / 2.0, 1.0))
    }
}

/// the tabs of the UprisingHack window.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum UprisingTab {
    #[default]
    KeyboardMouse,
    Controller,
    Presentation,
}

impl UprisingTab {
    const ALL: [(Self, &'static str); 3] = [
        (Self::KeyboardMouse, "Keyboard and mouse"),
        (Self::Controller, "Controller"),
        (Self::Presentation, "Presentation"),
    ];
}

impl Menus {
    /// is something open that stops the game while it is
    pub fn over_game(&self) -> bool {
        self.menu_open || self.settings_open || self.uprising_open || self.game_settings.is_some()
    }

    /// whether any Zakuro window is open over the game, the ones the game
    /// keeps running under included: the mouse is theirs, not the aim's.
    pub fn any_open(&self) -> bool {
        self.over_game() || self.editor_open || self.cheats_open || self.multiplayer_open || self.message.is_some()
    }

    /// the library, filling the window, waiting are the games waiting for their turn to recompile
    pub fn library(&mut self, ui: &mut egui::Ui, library: &Library, settings: &Settings, jobs: &[Job], waiting: &[Wanted]) -> Vec<Action> {
        let mut actions = Vec::new();
        if library.changed {
            self.icons.clear();
        }
        egui::Panel::top("library bar").show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("Zakuro");
                ui.separator();
                match &settings.games {
                    Some(folder) => ui.label(folder.display().to_string()),
                    None => ui.label(RichText::new("no games folder yet").weak()),
                };
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("UprisingHack").clicked() {
                        self.uprising_open = true;
                    }
                    if ui.button("Settings").clicked() {
                        self.settings_open = true;
                    }
                    if ui.button("Multiplayer").clicked() {
                        self.multiplayer_open = true;
                    }
                    if ui.add_enabled(settings.games.is_some(), egui::Button::new("Refresh")).clicked() {
                        actions.push(Action::Rescan);
                    }
                    if ui.button("Choose folder…").clicked() {
                        actions.push(Action::ChooseFolder);
                    }
                    let outdated = idle(library.games.iter().filter(|game| game.outdated).map(Game::wanted), jobs, waiting);
                    if !outdated.is_empty() {
                        let recompile = ui.button(format!("Recompile outdated ({})", outdated.len())).on_hover_text(
                            "These games were recompiled by an older version of Zakuro. Recompiling them again gives them its improvements. They go one at a time, around ten minutes each, and games can be played meanwhile.",
                        );
                        if recompile.clicked() {
                            actions.push(Action::Recompile(outdated));
                        }
                    }
                    if !waiting.is_empty() && ui.button(format!("Cancel the {} waiting", waiting.len())).on_hover_text("The game recompiling now carries on.").clicked() {
                        actions.push(Action::CancelWaiting);
                    }
                });
            });
            ui.add_space(6.0);
        });
        let background = match self.background.get(ui.ctx(), settings.background.as_deref()) {
            Ok(texture) => texture.map(|texture| (texture.id(), texture.size())),
            Err(error) => {
                self.message = Some(error);
                None
            }
        };
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some((texture, size)) = background {
                let area = ui.clip_rect();
                let alpha = (settings.background_opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
                ui.painter().image(texture, area, cover(area.size(), size), Color32::from_white_alpha(alpha));
            }
            if settings.games.is_none() {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() / 3.0);
                    ui.label("Pick the folder your games are in to see them here.");
                    if ui.button("Choose folder…").clicked() {
                        actions.push(Action::ChooseFolder);
                    }
                });
                return;
            }
            if library.is_scanning() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Looking for games…");
                });
                return;
            }
            if library.games.is_empty() {
                ui.label("No .3ds, .cci, .cxi or .cia files in that folder.");
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for game in &library.games {
                    let job = jobs.iter().find(|job| job.program_id == game.program_id);
                    // over a picture, each game gets a backing to keep it readable
                    let mut frame = egui::Frame::group(ui.style());
                    if background.is_some() {
                        frame = frame.fill(ui.visuals().panel_fill.gamma_multiply(0.85));
                    }
                    frame.show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let icon = game.icon.as_ref().map(|pixels| {
                                self.icons.entry(game.path.clone()).or_insert_with(|| {
                                    let image = egui::ColorImage::from_rgba_unmultiplied([ICON_SIZE, ICON_SIZE], pixels);
                                    ui.ctx().load_texture(game.path.display().to_string(), image, egui::TextureOptions::LINEAR)
                                })
                            });
                            match icon {
                                Some(texture) => {
                                    ui.add(egui::Image::new(&*texture).fit_to_exact_size(Vec2::splat(ICON_SIZE as f32)));
                                }
                                None => {
                                    ui.allocate_space(Vec2::splat(ICON_SIZE as f32));
                                }
                            }
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&game.name).strong().size(16.0));
                                if !game.publisher.is_empty() {
                                    ui.label(&game.publisher);
                                }
                                if let Some(problem) = &game.problem {
                                    ui.label(RichText::new(problem).color(Color32::from_rgb(230, 160, 80)));
                                    return;
                                }
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("{:016X}", game.program_id)).weak().monospace());
                                    if game.outdated {
                                        ui.label(RichText::new("recompiled by an older version").color(Color32::from_rgb(220, 180, 100)));
                                    } else if game.recompiled {
                                        ui.label(RichText::new("recompiled").color(Color32::from_rgb(110, 200, 120)));
                                    }
                                    if game.modded {
                                        ui.label(RichText::new("mods").color(Color32::from_rgb(120, 170, 230)));
                                    }
                                    if settings.has_own(game.program_id) {
                                        ui.label(RichText::new("own settings").color(Color32::from_rgb(120, 170, 230)));
                                    }
                                    if let Some(update) = &game.update {
                                        let version = zakuro_fs::version_name(update.version);
                                        ui.label(RichText::new(format!("update {version}")).color(Color32::from_rgb(220, 180, 100)));
                                    }
                                    if !game.dlc.is_empty() {
                                        ui.label(RichText::new("DLC").color(Color32::from_rgb(220, 180, 100)));
                                    }
                                });
                            });
                            if game.problem.is_some() {
                                return;
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button(RichText::new("▶ Play").size(15.0)).clicked() {
                                    actions.push(Action::Play(game.path.clone()));
                                }
                                match job {
                                    Some(job) if !job.stage().finished() => {
                                        if ui.button("Cancel").clicked() {
                                            actions.push(Action::CancelRecompile(job.program_id));
                                        }
                                        progress(ui, job);
                                    }
                                    _ if waiting.iter().any(|wanted| wanted.program_id == game.program_id) => {
                                        if ui.button("Cancel").clicked() {
                                            actions.push(Action::CancelRecompile(game.program_id));
                                        }
                                        ui.label(RichText::new("waiting to recompile").weak());
                                    }
                                    _ => {
                                        let label = if game.recompiled { "Recompile again" } else { "Recompile" };
                                        if ui.button(label).on_hover_text("Turn the game's code into native code with 3dsrecomp, which makes it run faster. It takes around ten minutes and needs a C compiler, gcc or clang, or Zig, which Zakuro offers to download when the computer has neither.").clicked() {
                                            actions.push(Action::Recompile(vec![game.wanted()]));
                                        }
                                        if let Some(Stage::Failed(error)) = job.map(Job::stage) {
                                            ui.label(RichText::new(format!("failed, {error}")).color(Color32::LIGHT_RED));
                                        }
                                    }
                                }
                                if ui.button("Mods").on_hover_text("Open the game's mods folder. Files put in its romfs folder replace the game's, the way Luma3DS and Citra take mods, and a texture pack made for Citra or Azahar goes in its textures folder.").clicked() {
                                    actions.push(Action::Mods(game.program_id));
                                }
                                if ui.button("Settings").on_hover_text("What this game has of its own, over the general settings.").clicked() {
                                    self.game_settings = Some((game.program_id, game.name.clone()));
                                }
                            });
                        });
                    });
                }
            });
        });
        actions
    }

    /// what goes over a running game, the menu when it's open and the frame rate when asked for
    #[allow(clippy::too_many_arguments)]
    pub fn game(
        &mut self,
        ui: &mut egui::Ui,
        name: &str,
        program_id: Option<u64>,
        fps: Option<FpsReadout>,
        recompiled: bool,
        fast: bool,
        jobs: &[Job],
        waiting: usize,
        saved: Option<&str>,
        rules: Option<EditorRules>,
        editor_layers: Vec<(String, [f32; 4])>,
        editor_visibility: Vec<bool>,
        navigation_controls: Vec<(usize, [Option<usize>; 4])>,
        navigation_directions: [bool; 4],
        editor_widen: f32,
        canvas: Option<[f32; 4]>,
        editor_open: bool,
        editor_enabled: bool,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        let ctx = ui.ctx().clone();
        if let Some((text, since)) = &self.toast {
            if since.elapsed() < std::time::Duration::from_millis(2500) {
                egui::Area::new(egui::Id::new("toast"))
                    .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 8.0))
                    .interactable(false)
                    .show(&ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.label(text);
                        });
                    });
                ctx.request_repaint();
            } else {
                self.toast = None;
            }
        }
        if fast {
            egui::Area::new(egui::Id::new("fast forward"))
                .anchor(egui::Align2::RIGHT_TOP, Vec2::new(-8.0, 8.0))
                .interactable(false)
                .show(&ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(RichText::new("» fast forward").monospace());
                    });
                });
        }
        if let Some(fps) = fps {
            // clicks go thru to the screen under it
            egui::Area::new(egui::Id::new("fps")).fixed_pos(egui::pos2(8.0, 8.0)).interactable(false).show(&ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new(fps.text()).monospace());
                });
            });
        }
        if self.menu_open {
            egui::Window::new("Paused")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(&ctx, |ui| {
                    ui.label(RichText::new(name).strong());
                    ui.label(RichText::new(if recompiled { "running its recompiled code" } else { "interpreted" }).weak());
                    ui.add_space(6.0);
                    let wide = |ui: &mut egui::Ui, text: &str| ui.add_sized([200.0, 28.0], egui::Button::new(text)).clicked();
                    if wide(ui, "Resume") {
                        actions.push(Action::Resume);
                    }
                    if wide(ui, "Reset") {
                        actions.push(Action::Reset);
                    }
                    if wide(ui, "Fullscreen") {
                        actions.push(Action::Fullscreen);
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut self.slot).range(1..=9).prefix("Slot "));
                        ui.label(RichText::new(saved.unwrap_or("empty")).weak());
                    });
                    ui.horizontal(|ui| {
                        if ui.add_sized([98.0, 28.0], egui::Button::new("Save state")).on_hover_text("F5 saves to this slot while playing.").clicked() {
                            actions.push(Action::SaveState);
                        }
                        if ui
                            .add_enabled(saved.is_some(), egui::Button::new("Load state").min_size(Vec2::new(98.0, 28.0)))
                            .on_hover_text("F8 loads this slot while playing.")
                            .clicked()
                        {
                            actions.push(Action::LoadState);
                        }
                    });
                    ui.add_space(4.0);
                    if editor_enabled && wide(ui, "Layout editor") {
                        self.editor_open = !self.editor_open;
                    }
                    if wide(ui, "Settings") {
                        self.settings_open = true;
                    }
                    if wide(ui, "UprisingHack") {
                        self.uprising_open = true;
                    }
                    if let Some(program_id) = program_id.filter(|_| wide(ui, "Settings for this game")) {
                        self.game_settings = Some((program_id, name.to_owned()));
                    }
                    if wide(ui, "Cheats") {
                        self.cheats_open = true;
                    }
                    if wide(ui, "Multiplayer") {
                        self.multiplayer_open = true;
                    }
                    if ui
                        .add_sized([200.0, 28.0], egui::Button::new("Show the camera a picture"))
                        .on_hover_text("Pick a picture for the console's cameras to see, a QR code's say, in place of the computer's camera.")
                        .clicked()
                    {
                        actions.push(Action::ChooseCameraPicture);
                    }
                    if wide(ui, "Copy info for a report") {
                        actions.push(Action::CopyReport);
                    }
                    if wide(ui, "Back to the library") {
                        actions.push(Action::Library);
                    }
                    if wide(ui, "Quit") {
                        actions.push(Action::Quit);
                    }
                    for job in jobs.iter().filter(|job| !job.stage().finished()) {
                        ui.separator();
                        ui.label(&job.name);
                        progress(ui, job);
                    }
                    if waiting > 0 {
                        ui.label(RichText::new(format!("{waiting} more waiting to recompile")).weak());
                    }
                });
        }
        self.layout_editor(&ctx, rules, editor_layers, editor_visibility, navigation_controls, navigation_directions, editor_widen, canvas, editor_open, &mut actions);
        actions
    }

    /// the dev-only layout editor: the edited Screen's layers on a canvas,
    /// and the edited Screen's rules, live, with reset and export.
    #[allow(clippy::too_many_arguments)]
    fn layout_editor(
        &mut self,
        ctx: &egui::Context,
        rules: Option<EditorRules>,
        layers: Vec<(String, [f32; 4])>,
        visibility: Vec<bool>,
        navigation_controls: Vec<(usize, [Option<usize>; 4])>,
        navigation_directions: [bool; 4],
        widen: f32,
        canvas: Option<[f32; 4]>,
        open: bool,
        actions: &mut Vec<Action>,
    ) -> NavigationEditorResponse {
        if !open {
            self.editor_resizing = false;
            self.editor_drag.clear();
            return NavigationEditorResponse::default();
        }
        // an index and a box belong to the layer list they were picked from
        if self.editor_seen.len() != layers.len() || self.editor_seen.iter().zip(&layers).any(|(seen, (name, _))| seen != name) {
            self.editor_seen = layers.iter().map(|(name, _)| name.clone()).collect();
            self.editor_layer = None;
            self.editor_group.clear();
            self.editor_resizing = false;
            self.editor_drag.clear();
            // an edit of another layer list cannot be undone on this one
            self.editor_undo.clear();
            self.editor_redo.clear();
            self.editor_renaming = None;
        }
        // this Screen's folders, kept to layers it has
        let mut folders: Vec<EditorFolder> = self.editor_screen.map_or(Vec::new(), |screen| {
            self.editor_folders.iter().filter(|folder| folder.screen == screen).cloned().collect()
        });
        for folder in &mut folders { folder.parts.retain(|&part| part < layers.len()); }
        folders.retain(|folder| !folder.parts.is_empty());
        let before = EditorSnapshot {
            layers: layers.iter().map(|&(_, to)| to).collect(),
            visibility: visibility.clone(),
            folders: folders.clone(),
        };
        let mut edited = false;
        let (mut undo, mut redo) = (false, false);
        if !ctx.text_edit_focused() {
            ctx.input_mut(|input| {
                let shortcut = |modifiers, key| egui::KeyboardShortcut::new(modifiers, key);
                // Shift first: a plain Ctrl+Z shortcut also matches Ctrl+Shift+Z
                redo = input.consume_shortcut(&shortcut(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z))
                    || input.consume_shortcut(&shortcut(egui::Modifiers::COMMAND, egui::Key::Y));
                undo = !redo && input.consume_shortcut(&shortcut(egui::Modifiers::COMMAND, egui::Key::Z));
            });
        }
        if self.editor_layer.is_some_and(|index| index >= layers.len()) {
            self.editor_layer = None;
            self.editor_group.clear();
            self.editor_resizing = false;
            self.editor_drag.clear();
        }
        // the list picks one layer; the group follows it unless it is a member
        if let Some(index) = self.editor_layer.filter(|index| !self.editor_group.contains(index)) {
            self.editor_group = vec![index];
        }
        if self.editor_layer.is_none() { self.editor_group.clear(); }
        self.editor_group = linked_group(&self.editor_group, &folders);
        if let Some([x, y, width, height]) = canvas.filter(|_| !layers.is_empty() && widen > 0.0) {
            let pixels = ctx.pixels_per_point();
            let area = egui::Rect::from_min_size(egui::pos2(x / pixels, y / pixels), egui::vec2(width / pixels, height / pixels));
            let scale_x = area.width() / (400.0 * widen);
            let scale_y = area.height() / 240.0;
            let mut selected = self.editor_layer;
            let mut group = std::mem::take(&mut self.editor_group);
            let mut resizing = self.editor_resizing;
            let mut drag = std::mem::take(&mut self.editor_drag);
            let mut moved = Vec::new();
            // a drag or nudge began: one edit to undo
            let mut started = false;
            let adding = ctx.input(|input| input.modifiers.command || input.modifiers.shift);
            // below the windows, so their lists and buttons keep the pointer
            egui::Area::new(egui::Id::new("layout canvas")).order(egui::Order::Background).fixed_pos(area.min).show(ctx, |ui| {
                let (canvas, response) = ui.allocate_exact_size(area.size(), egui::Sense::click_and_drag());
                let painter = ui.painter();
                let rect_of = |to: [f32; 4]| egui::Rect::from_min_size(
                    canvas.min + egui::vec2(to[0] * scale_x, to[1] * scale_y),
                    egui::vec2(to[2] * scale_x, to[3] * scale_y),
                );
                let handle_of = |to: [f32; 4]| egui::Rect::from_min_size(rect_of(to).max - egui::vec2(9.0, 9.0), egui::vec2(10.0, 10.0));
                let shown = |index: usize| visibility.get(index).copied().unwrap_or(true);
                let under = |point: egui::Pos2| layers.iter().enumerate().rev()
                    .find(|(index, (_, to))| shown(*index) && rect_of(*to).contains(point))
                    .map(|(index, _)| index);
                for (index, (name, to)) in layers.iter().enumerate() {
                    let rect = rect_of(*to);
                    let active = shown(index);
                    let picked = group.contains(&index);
                    let colour = if !active {
                        egui::Color32::from_gray(110)
                    } else if selected == Some(index) {
                        egui::Color32::from_rgb(0, 220, 255)
                    } else if picked {
                        egui::Color32::from_rgb(255, 200, 0)
                    } else {
                        egui::Color32::from_rgb(160, 200, 220)
                    };
                    painter.rect_stroke(rect, 0.0, egui::Stroke::new(if active && picked { 2.0 } else { 1.0 }, colour), egui::StrokeKind::Inside);
                    painter.text(rect.min + egui::vec2(2.0, 2.0), egui::Align2::LEFT_TOP, name, egui::FontId::monospace(10.0), colour);
                    // a hidden layer has no handle: it cannot be dragged until shown
                    if active {
                        painter.rect_filled(egui::Rect::from_min_size(rect.max - egui::vec2(7.0, 7.0), egui::vec2(7.0, 7.0)), 0.0, colour);
                    }
                }
                if response.clicked() {
                    if let Some(point) = response.interact_pointer_pos() {
                        match under(point) {
                            Some(index) if adding && group.contains(&index) => {
                                // a linked folder leaves the selection whole
                                let leaving = linked_group(&[index], &folders);
                                group.retain(|member| !leaving.contains(member));
                                selected = group.last().copied();
                            }
                            Some(index) if adding => {
                                group.push(index);
                                selected = Some(index);
                            }
                            hit => {
                                selected = hit;
                                group = hit.into_iter().collect();
                            }
                        }
                        group = linked_group(&group, &folders);
                        if selected.is_none() { resizing = false; }
                    }
                }
                if response.drag_started() {
                    // where the button went down, not where the pointer is now:
                    // a corner drag leaves the layer before egui calls it a drag
                    let point = ctx.input(|input| input.pointer.press_origin());
                    // a selected layer under the press is dragged even when
                    // another covers it, so one picked in the list can be moved
                    let picked = |point: egui::Pos2| selected.into_iter().chain(group.iter().copied())
                        .find(|&index| shown(index) && rect_of(layers[index].1).contains(point));
                    let hit = point.and_then(|point| picked(point).or_else(|| under(point)));
                    // dragging a member moves the whole group
                    match hit {
                        Some(index) if group.contains(&index) => {}
                        Some(index) if adding => group.push(index),
                        _ => group = hit.into_iter().collect(),
                    }
                    group = linked_group(&group, &folders);
                    started = hit.is_some();
                    selected = hit;
                    resizing = match (hit, point) {
                        (Some(index), Some(point)) => handle_of(layers[index].1).contains(point),
                        _ => false,
                    };
                    drag = group.iter().filter(|&&index| shown(index)).map(|&index| (index, layers[index].1)).collect();
                }
                if response.dragged() && selected.is_some() {
                    // the whole distance from the press, so a still pointer
                    // asks for the same boxes again and released moves stop
                    if let Some(by) = response.total_drag_delta() {
                        moved = dragged(&drag, [by.x / scale_x, by.y / scale_y], resizing);
                    }
                }
                if response.drag_stopped() {
                    resizing = false;
                    drag.clear();
                }
                // the arrow keys nudge the selection, Shift by ten, unless a
                // text field has the keyboard or a drag is going on
                if selected.is_some() && drag.is_empty() && !ctx.text_edit_focused() {
                    let (by, step) = ctx.input(|input| {
                        let pressed = |key| input.key_pressed(key) as i32 as f32;
                        let by = [pressed(egui::Key::ArrowRight) - pressed(egui::Key::ArrowLeft), pressed(egui::Key::ArrowDown) - pressed(egui::Key::ArrowUp)];
                        (by, if input.modifiers.shift { 10.0 } else { 1.0 })
                    });
                    if by != [0.0, 0.0] {
                        let start: Vec<_> = group.iter().filter(|&&index| shown(index)).map(|&index| (index, layers[index].1)).collect();
                        moved = dragged(&start, [by[0] * step, by[1] * step], false);
                        started = true;
                    }
                }
            });
            edited |= started;
            self.editor_layer = selected;
            self.editor_group = group;
            self.editor_resizing = resizing;
            self.editor_drag = drag;
            actions.extend(moved.into_iter().map(|(index, to)| Action::EditorLayer(index, to)));
        }
        let rules = match rules {
            Some(EditorRules::Cauldron(cauldron)) => {
                let list = self.cauldron_editor(ctx, cauldron, &layers, &visibility, actions);
                self.editor_history(before, folders, edited || list.edited, undo || list.undo, redo || list.redo, actions);
                return NavigationEditorResponse::default();
            }
            Some(EditorRules::Loading(loading)) => {
                let list = self.loading_editor(ctx, loading, &layers, &visibility, actions);
                self.editor_history(before, folders, edited || list.edited, undo || list.undo, redo || list.redo, actions);
                return NavigationEditorResponse::default();
            }
            Some(EditorRules::RetreatDialog(dialog)) => {
                self.dialog_editor(ctx, dialog, actions);
                self.editor_history(before, folders, edited, undo, redo, actions);
                return NavigationEditorResponse::default();
            }
            Some(EditorRules::Pause(pause)) => Some(pause),
            None => None,
        };
        let mut list = LayerListResponse::default();
        let Some(mut rules) = rules else {
            let mut window_open = true;
            let mut save = false;
            let mut export = false;
            let mut navigation = NavigationEditorResponse::default();
            let room = (ctx.content_rect().height() - 150.0).clamp(120.0, 420.0);
            egui::Window::new("Menu editor").id(egui::Id::new("menu_editor")).open(&mut window_open).default_width(380.0).show(ctx, |ui| {
                ui.label(if layers.is_empty() { "Open a Scripted Menu or the Pause menu to edit its layout." } else { "Select a layer on the canvas to move or resize it." });
                egui::ScrollArea::vertical().max_height(room).show(ui, |ui| {
                    list = self.editor_layer_list(ui, &layers, &visibility, &mut folders, actions);
                    navigation = navigation_editor(ui, &layers, &navigation_controls, navigation_directions, self.editor_layer, actions);
                });
                if !layers.is_empty() {
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Save layout").on_hover_text("Write the moved and hidden layers, folders and navigation links to menu-layout.toml.").clicked() {
                            save = true;
                        }
                        if ui.button("Export layers").clicked() {
                            export = true;
                        }
                    });
                }
            });
            if !window_open { self.editor_open = false; }
            #[cfg(test)]
            {
                navigation.visibility = std::mem::take(&mut list.boxes);
            }
            if export {
                self.message = Some(match export_layers(&layers, &visibility) {
                    Ok(()) => "Wrote editor-layers-export.txt".to_owned(),
                    Err(error) => format!("Could not write editor-layers-export.txt: {error}"),
                });
            }
            self.editor_history(before, folders, edited || list.edited, undo || list.undo, redo || list.redo, actions);
            if save { actions.push(Action::SaveLayout); }
            return navigation;
        };
        let mut window_open = true;
        let mut changed = false;
        let mut save = false;
        let mut message = None;
        let mut navigation = NavigationEditorResponse::default();
        let room = (ctx.content_rect().height() - 170.0).clamp(120.0, 420.0);
        egui::Window::new("Menu editor — Pause").id(egui::Id::new("menu_editor_pause")).open(&mut window_open).default_width(420.0).show(ctx, |ui| {
            ui.label(RichText::new("Edits apply live. F2 closes and reopens this window.").weak());
            egui::ScrollArea::vertical().max_height(room).show(ui, |ui| {
                changed |= rule_editor(ui, "span", &mut rules.span);
                changed |= rule_editor(ui, "center", &mut rules.center);
                changed |= rule_editor(ui, "left", &mut rules.left);
                changed |= rule_editor(ui, "right", &mut rules.right);
                changed |= rule_editor(ui, "corner", &mut rules.corner);
                list = self.editor_layer_list(ui, &layers, &visibility, &mut folders, actions);
                navigation = navigation_editor(ui, &layers, &navigation_controls, navigation_directions, self.editor_layer, actions);
                if !layers.is_empty() && ui.button("Export layers").clicked() {
                    message = Some(match export_layers(&layers, &visibility) {
                        Ok(()) => "Wrote editor-layers-export.txt".to_owned(),
                        Err(error) => format!("Could not write editor-layers-export.txt: {error}"),
                    });
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Save layout").on_hover_text("Write the pause rules, the moved and hidden layers, folders and navigation links to menu-layout.toml.").clicked() {
                    save = true;
                }
                if ui.button("Reset").clicked() {
                    rules = PauseRules::default();
                    changed = true;
                }
                if ui.button("Export Rust + change request").clicked() {
                    let text = format!("// Pause layout rules\n{}\n// Change request\n{}\n", rules.rust_fragment(), rules.change_request());
                    message = Some(match std::fs::write("menu-layout-export.txt", text) {
                        Ok(()) => "Wrote menu-layout-export.txt".to_owned(),
                        Err(error) => format!("Could not write menu-layout-export.txt: {error}"),
                    });
                }
            });
        });
        if !window_open { self.editor_open = false; }
        #[cfg(test)]
        {
            navigation.visibility = std::mem::take(&mut list.boxes);
        }
        self.editor_history(before, folders, edited || list.edited, undo || list.undo, redo || list.redo, actions);
        if changed { actions.push(Action::PauseRules(rules)); }
        if save { actions.push(Action::SaveLayout); }
        if let Some(message) = message { self.message = Some(message); }
        navigation
    }

    /// the layer list with this editor's selection and folder state.
    fn editor_layer_list(
        &mut self,
        ui: &mut egui::Ui,
        layers: &[(String, [f32; 4])],
        visibility: &[bool],
        folders: &mut Vec<EditorFolder>,
        actions: &mut Vec<Action>,
    ) -> LayerListResponse {
        let can_undo = !self.editor_undo.is_empty();
        let can_redo = !self.editor_redo.is_empty();
        layer_list(ui, layers, visibility, &mut self.editor_layer, &mut self.editor_group, folders, self.editor_screen, &mut self.editor_renaming, [can_undo, can_redo], actions)
    }

    /// keep this frame's folders, then step back or forward through the
    /// edits, or remember how the Screen was before an edit made now.
    /// `before` is the Screen as this frame found it, before its actions.
    fn editor_history(&mut self, before: EditorSnapshot, folders: Vec<EditorFolder>, edited: bool, undo: bool, redo: bool, actions: &mut Vec<Action>) {
        let edited = edited || folders != before.folders;
        let now = EditorSnapshot { folders, ..before.clone() };
        if undo || redo {
            // this frame's own edits give way to the step
            let (from, to) = if undo { (&mut self.editor_undo, &mut self.editor_redo) } else { (&mut self.editor_redo, &mut self.editor_undo) };
            // a snapshot like the Screen now (a drag that never moved) is no step
            while let Some(snapshot) = from.pop() {
                if snapshot == before { continue; }
                to.push(before.clone());
                for (index, (&was, &is)) in snapshot.layers.iter().zip(&before.layers).enumerate() {
                    if was != is { actions.push(Action::EditorLayer(index, was)); }
                }
                for (index, (&was, &is)) in snapshot.visibility.iter().zip(&before.visibility).enumerate() {
                    if was != is { actions.push(Action::EditorLayerVisible(index, was)); }
                }
                self.keep_folders(snapshot.folders);
                return;
            }
            self.keep_folders(before.folders);
            return;
        }
        if edited && self.editor_undo.last() != Some(&before) {
            self.editor_undo.push(before);
            if self.editor_undo.len() > UNDO_DEPTH { self.editor_undo.remove(0); }
            self.editor_redo.clear();
        }
        self.keep_folders(now.folders);
    }

    /// this Screen's folders in place of the ones kept for it.
    fn keep_folders(&mut self, folders: Vec<EditorFolder>) {
        let Some(screen) = self.editor_screen else { return };
        self.editor_folders.retain(|folder| folder.screen != screen);
        self.editor_folders.extend(folders);
        if self.editor_renaming.is_some_and(|folder| folder >= self.editor_folders.iter().filter(|folder| folder.screen == screen).count()) {
            self.editor_renaming = None;
        }
    }

    /// the Retreat Dialog's window: its dimming and its panel (box, buttons
    /// and words) as rules, then saved, reset or exported as Rust and a
    /// change request.
    fn dialog_editor(&mut self, ctx: &egui::Context, mut rules: DialogRules, actions: &mut Vec<Action>) {
        let mut window_open = true;
        let (mut changed, mut save) = (false, false);
        let mut message = None;
        let room = (ctx.content_rect().height() - 170.0).clamp(120.0, 420.0);
        egui::Window::new("Menu editor — Retreat Dialog").id(egui::Id::new("menu_editor_dialog")).open(&mut window_open).default_width(420.0).show(ctx, |ui| {
            ui.label(RichText::new("Edits apply live. The panel carries the box, its buttons and its words.").weak());
            egui::ScrollArea::vertical().max_height(room).show(ui, |ui| {
                changed |= rule_editor(ui, "span", &mut rules.span);
                changed |= rule_editor(ui, "panel", &mut rules.panel);
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Save layout").on_hover_text("Write the Retreat Dialog's rules with the rest of the layout to menu-layout.toml.").clicked() {
                    save = true;
                }
                if ui.button("Reset").clicked() {
                    rules = DialogRules::default();
                    changed = true;
                }
                if ui.button("Export Rust + change request").clicked() {
                    let text = format!("// Retreat Dialog layout rules\n{}\n// Change request\n{}\n", rules.rust_fragment(), rules.change_request());
                    message = Some(match std::fs::write("menu-layout-export.txt", text) {
                        Ok(()) => "Wrote menu-layout-export.txt".to_owned(),
                        Err(error) => format!("Could not write menu-layout-export.txt: {error}"),
                    });
                }
            });
        });
        if !window_open { self.editor_open = false; }
        if changed { actions.push(Action::DialogRules(rules)); }
        if save { actions.push(Action::SaveLayout); }
        if let Some(message) = message { self.message = Some(message); }
    }

    /// the Fiend's Cauldron's window: its eight parts listed with Visible
    /// checkboxes, moved and resized on the canvas, then saved, reset or
    /// exported as Rust and a change request.
    fn cauldron_editor(&mut self, ctx: &egui::Context, rules: CauldronRules, layers: &[(String, [f32; 4])], visibility: &[bool], actions: &mut Vec<Action>) -> LayerListResponse {
        let mut window_open = true;
        let (mut save, mut reset) = (false, false);
        let mut message = None;
        let mut list = LayerListResponse::default();
        let room = (ctx.content_rect().height() - 170.0).clamp(120.0, 420.0);
        egui::Window::new("Menu editor — Fiend's Cauldron").id(egui::Id::new("menu_editor_cauldron")).open(&mut window_open).default_width(380.0).show(ctx, |ui| {
            ui.label(RichText::new("Drag a part on the canvas to move it; its corner handle resizes it. Edits apply live.").weak());
            egui::ScrollArea::vertical().max_height(room).show(ui, |ui| {
                // the Cauldron is not a Scripted Menu, so it keeps no folders
                list = self.editor_layer_list(ui, layers, visibility, &mut Vec::new(), actions);
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Save layout").on_hover_text("Write the Fiend's Cauldron and Pause rules, the moved and hidden layers and navigation links to menu-layout.toml.").clicked() {
                    save = true;
                }
                if ui.button("Reset").clicked() {
                    reset = true;
                }
                if ui.button("Export Rust + change request").clicked() {
                    let text = format!("// Fiend's Cauldron layout rules\n{}\n// Change request\n{}\n", rules.rust_fragment(), rules.change_request());
                    message = Some(match std::fs::write("menu-layout-export.txt", text) {
                        Ok(()) => "Wrote menu-layout-export.txt".to_owned(),
                        Err(error) => format!("Could not write menu-layout-export.txt: {error}"),
                    });
                }
            });
        });
        if !window_open { self.editor_open = false; }
        if reset { actions.push(Action::CauldronRules(CauldronRules::default())); }
        if save { actions.push(Action::SaveLayout); }
        if let Some(message) = message { self.message = Some(message); }
        list
    }

    /// the Midstage Loading's window: its four parts listed with Visible
    /// checkboxes, moved and resized on the canvas, then saved, reset or
    /// exported as Rust and a change request.
    fn loading_editor(&mut self, ctx: &egui::Context, rules: LoadingRules, layers: &[(String, [f32; 4])], visibility: &[bool], actions: &mut Vec<Action>) -> LayerListResponse {
        let mut window_open = true;
        let (mut save, mut reset) = (false, false);
        let mut message = None;
        let mut list = LayerListResponse::default();
        let room = (ctx.content_rect().height() - 170.0).clamp(120.0, 420.0);
        egui::Window::new("Menu editor — Midstage Loading").id(egui::Id::new("menu_editor_loading")).open(&mut window_open).default_width(380.0).show(ctx, |ui| {
            ui.label(RichText::new("Drag a part on the canvas to move it; its corner handle resizes it. Edits apply live.").weak());
            egui::ScrollArea::vertical().max_height(room).show(ui, |ui| {
                // the loading is not a Scripted Menu, so it keeps no folders
                list = self.editor_layer_list(ui, layers, visibility, &mut Vec::new(), actions);
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Save layout").on_hover_text("Write the Midstage Loading's rules with the rest of the layout to menu-layout.toml.").clicked() {
                    save = true;
                }
                if ui.button("Reset").clicked() {
                    reset = true;
                }
                if ui.button("Export Rust + change request").clicked() {
                    let text = format!("// Midstage Loading layout rules\n{}\n// Change request\n{}\n", rules.rust_fragment(), rules.change_request());
                    message = Some(match std::fs::write("menu-layout-export.txt", text) {
                        Ok(()) => "Wrote menu-layout-export.txt".to_owned(),
                        Err(error) => format!("Could not write menu-layout-export.txt: {error}"),
                    });
                }
            });
        });
        if !window_open { self.editor_open = false; }
        if reset { actions.push(Action::LoadingRules(LoadingRules::default())); }
        if save { actions.push(Action::SaveLayout); }
        if let Some(message) = message { self.message = Some(message); }
        list
    }

    /// the cheats window over a game, when open, its cheats by name, if each is on, and their notes
    pub fn cheats(&mut self, ctx: &egui::Context, cheats: &[(String, bool, String, bool)]) -> Vec<Action> {
        let mut actions = Vec::new();
        let mut open = self.cheats_open;
        egui::Window::new("Cheats").open(&mut open).default_width(380.0).show(ctx, |ui| {
            let mut checkbox = |ui: &mut egui::Ui, index: usize, name: &str, enabled: bool, notes: &str| {
                let mut on = enabled;
                let mut response = ui.checkbox(&mut on, name);
                if !notes.is_empty() {
                    response = response.on_hover_text(notes);
                }
                if response.changed() {
                    actions.push(Action::Cheat(index, on));
                }
            };
            // Zakuro's own, tested on this build of the game
            if cheats.iter().any(|cheat| cheat.3) {
                ui.label(RichText::new("Enhancements").strong());
                for (index, (name, enabled, notes, _)) in cheats.iter().enumerate().filter(|(_, cheat)| cheat.3) {
                    checkbox(ui, index, name, *enabled, "");
                    // what it does to the game and who made it, in sight b4 it gets turned on
                    ui.indent(index, |ui| ui.label(RichText::new(notes).weak().small()));
                }
                ui.separator();
            }
            if !cheats.iter().any(|cheat| !cheat.3) {
                ui.label(RichText::new("No cheats yet. Add one below, or put the cheat file Citra or Azahar has for this game in the cheats folder.").weak());
            }
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for (index, (name, enabled, notes, _)) in cheats.iter().enumerate().filter(|(_, cheat)| !cheat.3) {
                    checkbox(ui, index, name, *enabled, notes);
                }
            });
            ui.separator();
            ui.label("Add a cheat");
            ui.add(egui::TextEdit::singleline(&mut self.cheat_name).hint_text("Name"));
            ui.add(egui::TextEdit::multiline(&mut self.cheat_code).hint_text("00000000 00000000").desired_rows(4).font(egui::TextStyle::Monospace));
            ui.horizontal(|ui| {
                if ui.button("Add").clicked() && !self.cheat_code.trim().is_empty() {
                    actions.push(Action::AddCheat(std::mem::take(&mut self.cheat_name), std::mem::take(&mut self.cheat_code)));
                }
                if ui.button("Reload").on_hover_text("Read the cheats again from their file, after changing it.").clicked() {
                    actions.push(Action::ReloadCheats);
                }
                if ui.button("Open the cheats folder").clicked() {
                    actions.push(Action::CheatsFolder);
                }
            });
        });
        self.cheats_open = open;
        actions
    }

    /// the multiplayer window, when open, the room the player's in, or joining one and opening one
    pub fn multiplayer(&mut self, ctx: &egui::Context, settings: &mut Settings, room: Option<&RoomView>) -> Vec<Action> {
        let mut actions = Vec::new();
        let before = settings.clone();
        let mut open = self.multiplayer_open;
        egui::Window::new("Multiplayer").open(&mut open).default_width(380.0).show(ctx, |ui| {
            ui.label(
                RichText::new("Local wireless play with others goes through a room, which one of you opens and the others join over the network.")
                    .weak(),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Nickname");
                ui.add(egui::TextEdit::singleline(&mut settings.nickname).char_limit(10).hint_text("Player"))
                    .on_hover_text("The name others see, and the console's own, which games show. It changes with the next game started.");
            });
            ui.separator();
            match room {
                Some(room) => {
                    ui.label(RichText::new(format!("In the room {}", room.name)).strong());
                    if let Some(port) = room.port {
                        ui.label(format!("Open on port {port}. Others join with this computer's address and that port. Over the internet the router has to let that port through."));
                    }
                    ui.add_space(4.0);
                    for (nickname, game) in &room.members {
                        match game.is_empty() {
                            true => ui.label(nickname),
                            false => ui.label(format!("{nickname}, playing {game}")),
                        };
                    }
                    ui.add_space(4.0);
                    let leave = if room.port.is_some() { "Close the room" } else { "Leave the room" };
                    if ui.button(leave).clicked() {
                        actions.push(Action::LeaveRoom);
                    }
                    if let Some(download_play) = &room.download_play {
                        ui.separator();
                        ui.label(RichText::new("Download Play").strong());
                        match &download_play.doing {
                            Some((doing, progress)) => {
                                ui.label(doing);
                                if let Some(progress) = progress {
                                    ui.add(egui::ProgressBar::new(*progress).show_percentage());
                                }
                                if ui.button("Leave").clicked() {
                                    actions.push(Action::DlpLeave);
                                }
                            }
                            None if download_play.offers.is_empty() => {
                                ui.label(RichText::new("No one in the room offers a game to download. A game offers one from its own Download Play menu.").weak());
                            }
                            None => {
                                for (index, offer) in download_play.offers.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        if let Some(pixels) = &offer.icon {
                                            let texture = self.dlp_icons.entry(offer.child).or_insert_with(|| {
                                                let image = egui::ColorImage::from_rgba_unmultiplied([ICON_SIZE, ICON_SIZE], pixels);
                                                ui.ctx().load_texture(format!("download play {:016X}", offer.child), image, egui::TextureOptions::LINEAR)
                                            });
                                            ui.add(egui::Image::new(&*texture).fit_to_exact_size(Vec2::splat(32.0)));
                                        }
                                        ui.vertical(|ui| {
                                            ui.label(RichText::new(&offer.title).strong());
                                            ui.label(RichText::new(format!("from {}, {} of {} joined", offer.host, offer.guests, offer.most)).weak());
                                        });
                                        if ui.add_enabled(offer.blocked.is_none(), egui::Button::new("Join")).clicked() {
                                            actions.push(Action::DlpJoin(index));
                                        }
                                    });
                                    if let Some(why) = &offer.blocked {
                                        ui.label(RichText::new(why).weak());
                                    }
                                }
                            }
                        }
                    }
                }
                None => {
                    ui.label(RichText::new("Join a room").strong());
                    ui.horizontal(|ui| {
                        ui.label("Address");
                        ui.add(egui::TextEdit::singleline(&mut settings.room_address).hint_text("192.168.0.10:24873"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Password");
                        ui.add(egui::TextEdit::singleline(&mut self.room_password).password(true).hint_text("none"));
                    });
                    if ui.add_enabled(!settings.room_address.trim().is_empty(), egui::Button::new("Join")).clicked() {
                        actions.push(Action::JoinRoom);
                    }
                    ui.separator();
                    ui.label(RichText::new("Open a room").strong());
                    ui.horizontal(|ui| {
                        ui.label("Name");
                        ui.add(egui::TextEdit::singleline(&mut settings.room_name).hint_text("Zakuro"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Port");
                        ui.add(egui::DragValue::new(&mut settings.room_port).range(1024..=65535));
                    });
                    ui.label(RichText::new("The password above, when there is one, is the room's.").weak());
                    if ui.button("Open").clicked() {
                        actions.push(Action::HostRoom);
                    }
                }
            }
        });
        self.multiplayer_open = open;
        if *settings != before {
            actions.push(Action::Settings);
        }
        actions
    }

    /// the settings window, when open, own says the running game has settings of its own, which go over these
    pub fn settings(&mut self, ctx: &egui::Context, settings: &mut Settings, own: bool) -> Vec<Action> {
        let mut actions = Vec::new();
        let before = settings.clone();
        let mut open = self.settings_open;
        // taller than a screen has room for, so it scrolls, and it can be dragged to any size that fits
        let room = ctx.content_rect().height() - 16.0;
        egui::Window::new("Settings")
            .id(egui::Id::new("settings"))
            .open(&mut open)
            .vscroll(true)
            .default_size([460.0, room])
            .max_height(room)
            .show(ctx, |ui| {
                if own {
                    ui.label(RichText::new("This game has settings of its own, which go over these. They are in Settings for this game, in the menu.").weak());
                }
                ui.heading("Games");
                ui.horizontal(|ui| {
                    match &settings.games {
                        Some(folder) => ui.label(folder.display().to_string()),
                        None => ui.label(RichText::new("none").weak()),
                    };
                    if ui.button("Choose…").clicked() {
                        actions.push(Action::ChooseFolder);
                    }
                });

                ui.separator();
                ui.heading("Graphics");
                egui::ComboBox::from_label("Presented with")
                    .selected_text(match settings.renderer {
                        Renderer::Vulkan => "Vulkan",
                        Renderer::OpenGl => "OpenGL",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut settings.renderer, Renderer::Vulkan, "Vulkan");
                        ui.selectable_value(&mut settings.renderer, Renderer::OpenGl, "OpenGL");
                    });
                rasterizer(ui, &mut settings.hardware_rasterizer);
                ui.add_enabled_ui(settings.hardware_rasterizer, |ui| {
                    resolution(ui, &mut settings.resolution);
                    texture_packs(ui, &mut settings.texture_packs);
                });
                window_scale(ui, &mut settings.scale);
                screens(ui, &mut settings.layout);
                rotation(ui, &mut settings.orientation);
                filter(ui, &mut settings.filter);
                integer_scale(ui, &mut settings.integer_scale);
                top_shape(ui, &mut settings.top_shape);
                ui.label(
                    RichText::new("F9 switches them while playing, F10 between the top and the bottom screen alone.").weak(),
                );
                ui.label(RichText::new("The presenter changes the next time Zakuro starts.").weak());

                ui.separator();
                ui.heading("Emulation");
                language(ui, &mut settings.language);
                egui::ComboBox::from_label("Cameras").selected_text(settings.camera.name()).show_ui(ui, |ui| {
                    for source in CameraSource::ALL {
                        ui.selectable_value(&mut settings.camera, source, source.name());
                    }
                })
                .response
                .on_hover_text("What the console's cameras see. The computer's camera turns on only while a game takes pictures.");
                if settings.camera == CameraSource::Picture {
                    ui.horizontal(|ui| {
                        ui.label("Picture");
                        match &settings.camera_picture {
                            Some(path) => ui.label(path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()),
                            None => ui.label(RichText::new("none").weak()),
                        };
                        if ui.button("Choose…").clicked() {
                            actions.push(Action::ChooseCameraPicture);
                        }
                    });
                }
                egui::ComboBox::from_label("Microphone").selected_text(settings.microphone.name()).show_ui(ui, |ui| {
                    for source in MicrophoneSource::ALL {
                        ui.selectable_value(&mut settings.microphone, source, source.name());
                    }
                })
                .response
                .on_hover_text("What the console's microphone hears. The computer's microphone is open only while a game listens, which some do all the time, Super Mario 3D Land among them. Holding M blows into it either way, for games that ask you to.");
                code(ui, &mut settings.recompiled);
                ui.label(RichText::new("It changes with the next game started, or Reset.").weak());

                ui.separator();
                ui.heading("Sound");
                ui.add(egui::Slider::new(&mut settings.volume, 0.0..=1.0).text("Volume").show_value(false));
                ui.checkbox(&mut settings.mute, "Mute");

                ui.separator();
                ui.heading("Interface");
                ui.checkbox(&mut settings.show_fps, "Show the frame rate");
                ui.horizontal(|ui| {
                    ui.label("Library background");
                    match &settings.background {
                        Some(path) => ui.label(path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()),
                        None => ui.label(RichText::new("none").weak()),
                    };
                    if ui.button("Choose…").clicked() {
                        actions.push(Action::ChooseBackground);
                    }
                    if settings.background.is_some() && ui.button("Remove").clicked() {
                        settings.background = None;
                    }
                });
                ui.add_enabled(
                    settings.background.is_some(),
                    egui::Slider::new(&mut settings.background_opacity, 0.0..=1.0)
                        .text("Opacity")
                        .custom_formatter(|value, _| format!("{:.0}%", value * 100.0)),
                );


                ui.separator();
                ui.heading("Controls");
                egui::Grid::new("controls").num_columns(4).spacing([12.0, 4.0]).show(ui, |ui| {
                    for (i, (name, key)) in settings.keys.all_mut().into_iter().enumerate() {
                        ui.label(name);
                        let text = if self.rebinding == Some(i) { "press a key…".to_owned() } else { key_name(*key) };
                        if ui.add_sized([110.0, 20.0], egui::Button::new(text)).clicked() {
                            self.rebinding = Some(i);
                            self.rebinding_pad = None;
                        }
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
                if ui.button("Default controls").clicked() {
                    settings.keys = Keys::default();
                    self.rebinding = None;
                }
                ui.label(RichText::new("Drag with the right mouse button to tilt the console, for games that use the motion sensors.").weak());

                ui.separator();
                ui.heading("Controller");
                egui::Grid::new("controller").num_columns(4).spacing([12.0, 4.0]).show(ui, |ui| {
                    for (i, (name, button)) in settings.pad.all_mut().into_iter().enumerate() {
                        ui.label(name);
                        let text = if self.rebinding_pad == Some(i) { "press a button…" } else { button_name(*button) };
                        if ui.add_sized([110.0, 20.0], egui::Button::new(text)).clicked() {
                            self.rebinding_pad = Some(i);
                            self.rebinding = None;
                        }
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
                ui.label(RichText::new("The circle pad is the left stick, and Home opens the menu over the game.").weak());
                if ui.button("Default controller").clicked() {
                    settings.pad = PadButtons::default();
                    self.rebinding_pad = None;
                }

            });
        self.settings_open = open;
        if !open {
            self.rebinding = None;
            self.rebinding_pad = None;
        }
        if *settings != before {
            actions.push(Action::Settings);
        }
        actions
    }

    /// whether the Settings or UprisingHack window is open.
    pub fn settings_shown(&self) -> bool {
        self.settings_open || self.uprising_open
    }

    /// the UprisingHack window, when open: the settings this port adds
    /// for Kid Icarus: Uprising, apart from the emulator's own Settings.
    pub fn uprising(&mut self, ctx: &egui::Context, settings: &mut Settings) -> Vec<Action> {
        let mut actions = Vec::new();
        let before = settings.clone();
        let mut open = self.uprising_open;
        let room = ctx.content_rect().height() - 16.0;
        egui::Window::new("UprisingHack")
            .id(egui::Id::new("uprising_hack"))
            .open(&mut open)
            .vscroll(true)
            .default_size([460.0, room])
            .max_height(room)
            .show(ctx, |ui| self.kid_icarus(ui, settings));
        self.uprising_open = open;
        if !open {
            self.rebinding_action = None;
            self.binding_conflict = None;
            self.pending_reset = None;
        }
        if *settings != before {
            actions.push(Action::Settings);
        }
        actions
    }

    /// Kid Icarus's PC Preferences, shown in the UprisingHack window.
    fn kid_icarus(&mut self, ui: &mut egui::Ui, settings: &mut Settings) {
        use crate::bindings::{Device, Resolve, Section};
        use crate::preferences::{Category, Presentation};
        ui.heading("Kid Icarus: Uprising");
        ui.label(RichText::new("The PC Preferences UprisingHack adds. Escape cancels a binding.").weak());
        if !settings.pc.review.is_empty() {
            ui.label(
                RichText::new(format!(
                    "Your old {} bindings have no single action now; check the actions below.",
                    settings.pc.review.join(", ")
                ))
                .color(Color32::YELLOW),
            );
            if ui.button("Got it").clicked() {
                settings.pc.review.clear();
            }
        }
        if let Some((action, slot, source, others)) = self.binding_conflict.clone() {
            let names: Vec<&str> = others.iter().map(|other| other.name()).collect();
            ui.label(RichText::new(format!("{} is already {}.", source.name(), names.join(", "))).color(Color32::YELLOW));
            ui.horizontal(|ui| {
                let mut resolve = None;
                if ui.button("Swap").clicked() {
                    resolve = Some(Resolve::Swap);
                }
                if ui.button("Replace").clicked() {
                    resolve = Some(Resolve::Replace);
                }
                if ui.button("Cancel").clicked() {
                    self.binding_conflict = None;
                }
                if let Some(resolve) = resolve {
                    self.binding_conflict = None;
                    if let Err(error) = settings.pc.bindings.assign(action, slot, source, resolve) {
                        self.message = Some(binding_refused(error));
                    }
                }
            });
        }
        ui.horizontal(|ui| {
            for (tab, name) in UprisingTab::ALL {
                ui.selectable_value(&mut self.uprising_tab, tab, name);
            }
        });
        ui.separator();
        let tab = self.uprising_tab;
        let device = match tab {
            UprisingTab::KeyboardMouse => Some((Device::KeyboardMouse, "Keyboard and mouse", "keyboard_mouse")),
            UprisingTab::Controller => Some((Device::Controller, "Controller", "controller")),
            UprisingTab::Presentation => None,
        };
        if let Some((device, title, key)) = device {
            for section in Section::ALL {
                let id = format!("{key}/{}", section.name().to_lowercase());
                let folded = settings.folded_sections.contains(&id);
                let response = egui::CollapsingHeader::new(RichText::new(section.name()).strong())
                    .id_salt(&id)
                    .open(Some(!folded))
                    .show(ui, |ui| {
                        self.binding_rows(ui, settings, device, section, &id);
                        self.section_settings(ui, settings, device, section);
                    });
                // the folds are kept with the settings, across runs
                if response.header_response.clicked() {
                    if folded {
                        settings.folded_sections.retain(|shut| shut != &id);
                    } else {
                        settings.folded_sections.push(id);
                    }
                }
            }
            ui.separator();
            let category = if device == Device::KeyboardMouse { Category::KeyboardMouse } else { Category::Controller };
            self.reset_button(ui, settings, category, &format!("Default {}", title.to_lowercase()));
            ui.label(RichText::new("Right-click a binding to clear it.").weak());
        }
        if tab == UprisingTab::Presentation {
        let shown = &mut settings.pc.presentation;
        ui.add(egui::Slider::new(&mut shown.hud_scale, Presentation::HUD.0..=Presentation::HUD.1).text("HUD and reticle size"));
        ui.add(egui::Slider::new(&mut shown.portrait_scale, Presentation::PORTRAIT.0..=Presentation::PORTRAIT.1).text("Portrait size"));
        ui.add(
            egui::Slider::new(&mut shown.land_camera_distance, Presentation::LAND_CAMERA.0..=Presentation::LAND_CAMERA.1)
                .text("Land Battle camera distance")
                .custom_formatter(|scale, _| format!("{:.0}%", scale * 100.0)),
        )
        .on_hover_text("How far behind Pit the camera stays on the ground. 100% is the game's own.");
        self.reset_button(ui, settings, Category::Presentation, "Default presentation");
        }
    }

    /// one section's actions on a device: the name, then the primary and
    /// secondary input, each a button to rebind and right-click to clear.
    fn binding_rows(&mut self, ui: &mut egui::Ui, settings: &mut Settings, device: crate::bindings::Device, section: crate::bindings::Section, id: &str) {
        use crate::bindings::Action as Act;
        // every section's columns as wide, so the inputs line up
        egui::Grid::new(id).num_columns(3).spacing([12.0, 4.0]).min_col_width(120.0).show(ui, |ui| {
            for action in section.actions(device) {
                // held, Walk runs when the keys walk by themselves
                let reversed = action == Act::Walk && settings.pc.movement.walk_by_default;
                ui.label(if reversed { "Run (hold)" } else { action.name() });
                for slot in 0..2 {
                    let bound = settings.pc.bindings.sources(device, action).get(slot).copied();
                    let text = match (self.rebinding_action == Some((action, device, slot)), bound) {
                        (true, _) => "press…".to_owned(),
                        (false, Some(source)) => source.name(),
                        (false, None) => "—".to_owned(),
                    };
                    let button = ui.add_sized([120.0, 20.0], egui::Button::new(text));
                    if button.clicked() {
                        self.rebinding_action = Some((action, device, slot));
                        self.binding_conflict = None;
                    }
                    if button.secondary_clicked() && bound.is_some() {
                        if let Err(error) = settings.pc.bindings.clear(device, action, slot) {
                            self.message = Some(binding_refused(error));
                        }
                    }
                }
                ui.end_row();
            }
        });
    }

    /// the choices a section has on a device besides its bindings: Walk's
    /// on the keyboard's Movement, each device's aim feel in its Aim, the
    /// Idol View's orbit in the keyboard's Idols.
    fn section_settings(&mut self, ui: &mut egui::Ui, settings: &mut Settings, device: crate::bindings::Device, section: crate::bindings::Section) {
        use crate::bindings::{Device, Section};
        use crate::preferences::{GyroMode, Movement};
        let aim = &mut settings.pc.aim;
        match (device, section) {
            (Device::KeyboardMouse, Section::Movement) => {
                let (low, high) = Movement::WALK_LEAN;
                ui.add(
                    egui::Slider::new(&mut settings.pc.movement.walk_lean, low..=high)
                        .text("Walk speed")
                        .custom_formatter(|lean, _| format!("{:.0}%", lean * 100.0)),
                )
                .on_hover_text("How far the movement keys push the stick while walking. Higher is faster; Pit never dashes while walking.");
                ui.checkbox(&mut settings.pc.movement.walk_by_default, "Walk by default")
                    .on_hover_text("The movement keys walk by themselves, and holding Run makes Pit run, dash and sprint.");
            }
            (Device::KeyboardMouse, Section::Aim) => {
                ui.add(egui::Slider::new(&mut aim.air_mouse_sensitivity, 0.1..=3.0).text("Air Battle mouse sensitivity"));
                ui.add(egui::Slider::new(&mut aim.land_mouse_sensitivity, 0.1..=3.0).text("Land Battle mouse sensitivity"));
                ui.horizontal(|ui| {
                    ui.checkbox(&mut aim.mouse_invert_x, "Invert mouse horizontally");
                    ui.checkbox(&mut aim.mouse_invert_y, "Invert mouse vertically");
                });
            }
            (Device::KeyboardMouse, Section::Idols) => {
                ui.add(egui::Slider::new(&mut aim.idol_view_sensitivity, 0.25..=3.0).text("Idol View orbit sensitivity"));
            }
            (Device::Controller, Section::Aim) => {
                ui.add(egui::Slider::new(&mut aim.stick_sensitivity, 0.25..=3.0).text("Right stick sensitivity"));
                ui.add(egui::Slider::new(&mut aim.dead_zone, 0.0..=0.5).text("Stick dead zone"));
                ui.horizontal(|ui| {
                    ui.checkbox(&mut aim.stick_invert_x, "Invert right stick horizontally");
                    ui.checkbox(&mut aim.stick_invert_y, "Invert right stick vertically");
                });
                ui.horizontal(|ui| {
                    ui.label("Gyro aim");
                    for (mode, name) in [(GyroMode::Off, "Off"), (GyroMode::Always, "Always on"), (GyroMode::Hold, "Hold to aim")] {
                        ui.radio_value(&mut aim.gyro, mode, name);
                    }
                });
                ui.add_enabled(aim.gyro != GyroMode::Off, egui::Slider::new(&mut aim.gyro_sensitivity, 0.25..=4.0).text("Gyro sensitivity"));
                ui.horizontal(|ui| {
                    ui.checkbox(&mut aim.gyro_invert_x, "Invert gyro horizontally");
                    ui.checkbox(&mut aim.gyro_invert_y, "Invert gyro vertically");
                    if ui.button("Calibrate").on_hover_text("Lay the controller still for a second").clicked() {
                        self.calibrate_gyro = true;
                    }
                });
                ui.label(RichText::new(&self.gyro_status).weak());
            }
            _ => {}
        }
    }

    /// a Restore Defaults button for one category, asking before it does.
    fn reset_button(&mut self, ui: &mut egui::Ui, settings: &mut Settings, category: crate::preferences::Category, text: &str) {
        if self.pending_reset != Some(category) {
            if ui.button(text).clicked() {
                self.pending_reset = Some(category);
            }
            return;
        }
        ui.horizontal(|ui| {
            ui.label(format!("{text}?"));
            if ui.button("Restore").clicked() {
                category.restore_defaults(settings);
                self.rebinding_action = None;
                self.pending_reset = None;
            }
            if ui.button("Cancel").clicked() {
                self.pending_reset = None;
            }
        });
    }
}

/// why an Action Binding was not changed.
fn binding_refused(error: crate::bindings::BindError) -> String {
    match error {
        crate::bindings::BindError::Required(action) => format!("{} must keep an input on each device.", action.name()),
        crate::bindings::BindError::NotOnDevice(action) => format!("{} takes keyboard and mouse inputs only.", action.name()),
        crate::bindings::BindError::NoSuchSlot => "There are two inputs an action.".to_owned(),
    }
}

impl Menus {

    /// the window of what a game has of its own, when open
    pub fn game_settings(&mut self, ctx: &egui::Context, settings: &mut Settings) -> Vec<Action> {
        let mut actions = Vec::new();
        let Some((program_id, name)) = self.game_settings.clone() else { return actions };
        let key = game_key(program_id);
        let mut game = settings.per_game.get(&key).cloned().unwrap_or_default();
        let before = game.clone();
        let general: &Settings = settings;
        let mut open = true;
        let room = ctx.content_rect().height() - 16.0;
        egui::Window::new(format!("Settings for {name}"))
            .id(egui::Id::new("game settings"))
            .open(&mut open)
            .vscroll(true)
            .default_size([420.0, room.min(540.0)])
            .max_height(room)
            .show(ctx, |ui| {
                ui.label(RichText::new("Tick what this game is to have of its own. The rest follows the general settings.").weak());
                ui.separator();
                ui.heading("Graphics");
                own(ui, &mut game.hardware_rasterizer, &general.hardware_rasterizer, rasterizer);
                own(ui, &mut game.resolution, &general.resolution, resolution);
                own(ui, &mut game.texture_packs, &general.texture_packs, texture_packs);
                own(ui, &mut game.scale, &general.scale, window_scale);
                own(ui, &mut game.layout, &general.layout, screens);
                own(ui, &mut game.orientation, &general.orientation, rotation);
                own(ui, &mut game.filter, &general.filter, filter);
                own(ui, &mut game.integer_scale, &general.integer_scale, integer_scale);
                own(ui, &mut game.top_shape, &general.top_shape, top_shape);
                ui.separator();
                ui.heading("Emulation");
                own(ui, &mut game.language, &general.language, language);
                own(ui, &mut game.recompiled, &general.recompiled, code);
                ui.separator();
                ui.label(RichText::new("Where the 3D is drawn, its resolution, the texture packs, the language and what runs the code change the next time the game starts, or with Reset.").weak());
                if ui.add_enabled(game != GameSettings::default(), egui::Button::new("Follow the general settings in everything")).clicked() {
                    game = GameSettings::default();
                }
            });
        if !open {
            self.game_settings = None;
        }
        if game != before {
            match game == GameSettings::default() {
                true => drop(settings.per_game.remove(&key)),
                false => drop(settings.per_game.insert(key, game)),
            }
            actions.push(Action::Settings);
        }
        actions
    }

    /// a message box, when there's something to tell
    /// the keyboard a game opened to get text typed in, it waits till a button closes it
    pub fn keyboard(&mut self, ctx: &egui::Context, request: &Request) -> Option<Action> {
        let fresh = self.typing.as_ref().is_none_or(|(asked, _)| asked != request);
        if fresh {
            self.typing = Some((request.clone(), request.text.clone()));
        }
        let (_, text) = self.typing.as_mut()?;
        let confirm = request.buttons.len() - 1;
        let mut action = None;
        egui::Window::new("Keyboard")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                let field = ui.add(
                    egui::TextEdit::singleline(text)
                        .hint_text(&request.hint)
                        .char_limit(request.max_length)
                        .desired_width(260.0),
                );
                if fresh {
                    field.request_focus();
                }
                let valid = request.accepts(text);
                ui.horizontal(|ui| {
                    for (button, label) in request.buttons.iter().enumerate() {
                        if ui.add_enabled(button != confirm || valid, egui::Button::new(label)).clicked() {
                            action = Some(Action::Keyboard(text.clone(), button));
                        }
                    }
                });
                // enter confirms, when the text will do
                if valid && field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                    action = Some(Action::Keyboard(text.clone(), confirm));
                }
            });
        if action.is_some() {
            self.typing = None;
        }
        action
    }

    /// asks whether to download a compiler, since there's none to recompile w/
    pub fn compiler_offer(&mut self, ctx: &egui::Context) -> Option<Action> {
        let games = self.compiler_offer.clone()?;
        let size = crate::zig::download_size().map_or_else(String::new, |size| format!(", {} MB to download and around 400 MB once unpacked", size / 1_000_000));
        let mut action = None;
        egui::Window::new("Recompile")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Recompiling needs a C compiler, and Zakuro didn't find one on this computer. It can download Zig, a compiler that needs no installing, into its own folder{size}."
                ));
                ui.horizontal(|ui| {
                    if ui.button("Download it and recompile").clicked() {
                        action = Some(Action::DownloadCompiler(games));
                        self.compiler_offer = None;
                    }
                    if ui.button("Not now").clicked() {
                        self.compiler_offer = None;
                    }
                });
            });
        action
    }

    pub fn message(&mut self, ctx: &egui::Context) {
        let Some(text) = self.message.clone() else { return };
        // a line of recompiles that all fail makes it long, and it scrolls to keep the OK that closes it on the screen
        let room = (ctx.content_rect().height() - 120.0).max(60.0);
        egui::Window::new("Zakuro")
            .id(egui::Id::new("message"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(room).show(ui, |ui| ui.label(text.as_str()));
                ui.horizontal(|ui| {
                    if let Some((_, report)) = self.report.as_ref().filter(|(about, _)| *about == text) {
                        if ui.button("Copy info for a report").clicked() {
                            ui.ctx().copy_text(report.clone());
                        }
                    }
                    if ui.button("OK").clicked() {
                        self.message = None;
                        self.report = None;
                    }
                });
            });
    }
}

/// a row of a game's own settings
/// ticked, the game has a value of its own, starting from the general one
/// unticked, it follows the general one, which shows greyed out
fn own<T: Clone>(ui: &mut egui::Ui, own: &mut Option<T>, general: &T, row: impl FnOnce(&mut egui::Ui, &mut T)) {
    ui.horizontal(|ui| {
        let mut ticked = own.is_some();
        if ui.checkbox(&mut ticked, "").on_hover_text("This game's own, rather than the general setting.").changed() {
            *own = ticked.then(|| general.clone());
        }
        let mut value = own.clone().unwrap_or_else(|| general.clone());
        ui.add_enabled_ui(ticked, |ui| row(ui, &mut value));
        if ticked {
            *own = Some(value);
        }
    });
}

fn rasterizer(ui: &mut egui::Ui, value: &mut bool) {
    ui.checkbox(value, "Draw the 3D on the GPU");
}

fn resolution(ui: &mut egui::Ui, value: &mut u32) {
    ui.add(egui::Slider::new(value, 1..=8).text("Resolution").suffix("x"))
        .on_hover_text("How many times the console's resolution the 3D is drawn at. Past what the window shows, the extra pixels smooth the edges, at a cost to the GPU that grows fast.");
}

fn texture_packs(ui: &mut egui::Ui, value: &mut bool) {
    ui.checkbox(value, "Texture packs")
        .on_hover_text("Draw a game's texture pack, made for Citra or Azahar, put in the textures folder of the game's mods folder.");
}

fn window_scale(ui: &mut egui::Ui, value: &mut u32) {
    ui.add(egui::Slider::new(value, 1..=6).text("Window scale"));
}

fn screens(ui: &mut egui::Ui, value: &mut Screens) {
    egui::ComboBox::from_label("Screens").selected_text(value.name()).show_ui(ui, |ui| {
        for layout in Screens::ALL {
            ui.selectable_value(value, layout, layout.name());
        }
    });
}

fn rotation(ui: &mut egui::Ui, value: &mut Orientation) {
    egui::ComboBox::from_label("Rotation").selected_text(value.name()).show_ui(ui, |ui| {
        for orientation in Orientation::ALL {
            ui.selectable_value(value, orientation, orientation.name());
        }
    });
}

fn filter(ui: &mut egui::Ui, value: &mut Filter) {
    egui::ComboBox::from_label("Filter")
        .selected_text(value.name())
        .show_ui(ui, |ui| {
            for filter in Filter::ALL {
                ui.selectable_value(value, filter, filter.name());
            }
        })
        .response
        .on_hover_text("How the screens' pixels look scaled up: smooth, sharp with soft edges, or plain squares.");
}

fn integer_scale(ui: &mut egui::Ui, value: &mut bool) {
    ui.checkbox(value, "Scale by whole numbers only").on_hover_text("Every pixel gets the same size, with a border around the screens.");
}

fn top_shape(ui: &mut egui::Ui, value: &mut TopShape) {
    egui::ComboBox::from_label("Top screen shape")
        .selected_text(value.name())
        .show_ui(ui, |ui| {
            for shape in TopShape::ALL {
                ui.selectable_value(value, shape, shape.name());
            }
        })
        .response
        .on_hover_text("Shows the top screen wider than the console's. Games still draw it at the console's shape, so only turn this on with a widescreen code for the game, which draws a wider scene to match. Kid Icarus: Uprising draws a wider view by itself. The window's shape follows the window, with the top screen alone, up to 32:9.");
}


fn language(ui: &mut egui::Ui, value: &mut Language) {
    egui::ComboBox::from_label("Language")
        .selected_text(value.name())
        .show_ui(ui, |ui| {
            for language in Language::ALL {
                ui.selectable_value(value, language, language.name());
            }
        })
        .response
        .on_hover_text("The console's system language, which titles pick their localised text and files by. It changes with the next game started, or Reset.");
}

/// what runs the game's code
fn code(ui: &mut egui::Ui, recompiled: &mut bool) {
    ui.radio_value(recompiled, true, "Recompiled code when there is some");
    ui.radio_value(recompiled, false, "Interpreter only");
}

/// a line a GameOverlay keeps over the game, in the bottom right corner
pub fn corner(ctx: &egui::Context, text: &str) {
    egui::Area::new(egui::Id::new("overlay corner"))
        .anchor(egui::Align2::RIGHT_BOTTOM, Vec2::new(-8.0, -8.0))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(text);
            });
        });
}

/// the games that aren't recompiling or waiting to, each once, the same game being in the folder twice as a .3ds and a .cci say
fn idle(games: impl Iterator<Item = Wanted>, jobs: &[Job], waiting: &[Wanted]) -> Vec<Wanted> {
    let busy = |id: u64| waiting.iter().any(|wanted| wanted.program_id == id) || jobs.iter().any(|job| job.program_id == id && !job.stage().finished());
    let mut idle: Vec<Wanted> = Vec::new();
    for game in games {
        if !busy(game.program_id) && !idle.iter().any(|wanted| wanted.program_id == game.program_id) {
            idle.push(game);
        }
    }
    idle
}

/// a recompile's progress bar w/ where it is
fn progress(ui: &mut egui::Ui, job: &Job) {
    let stage = job.stage();
    let text = match &stage {
        Stage::Downloading(arrived) => format!("downloading the compiler, {:.0}%", arrived * 100.0),
        Stage::Generating => "finding the code".to_owned(),
        Stage::Compiling { done, total } => format!("compiling {done} of {total}"),
        Stage::Installing => "installing".to_owned(),
        Stage::Done => "done".to_owned(),
        Stage::Failed(error) => format!("failed, {error}"),
    };
    let minutes = job.elapsed().as_secs() / 60;
    let seconds = job.elapsed().as_secs() % 60;
    ui.add(egui::ProgressBar::new(stage.fraction()).desired_width(220.0).text(format!("{text}, {minutes}:{seconds:02}")));
}

/// a key as a person would call it
pub fn key_name(key: KeyCode) -> String {
    let name = format!("{key:?}");
    for prefix in ["Key", "Digit", "Arrow"] {
        if let Some(rest) = name.strip_prefix(prefix) {
            return rest.to_owned();
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_background_fills_the_area_without_stretching() {
        // a square picture in a wide area loses its top and bottom
        let wide = cover(Vec2::new(200.0, 100.0), [100, 100]);
        assert_eq!((wide.min.x, wide.max.x), (0.0, 1.0));
        assert_eq!((wide.min.y, wide.max.y), (0.25, 0.75));
        // and in a tall one its sides
        let tall = cover(Vec2::new(100.0, 200.0), [100, 100]);
        assert_eq!((tall.min.x, tall.max.x), (0.25, 0.75));
        assert_eq!((tall.min.y, tall.max.y), (0.0, 1.0));
    }

    #[test]
    fn big_pictures_are_scaled_down_keeping_their_shape() {
        let path = std::env::temp_dir().join(format!("zakuro-background-{}.png", std::process::id()));
        image::RgbaImage::from_pixel(BACKGROUND_SIDE * 2, BACKGROUND_SIDE, image::Rgba([10, 20, 30, 255])).save(&path).unwrap();
        let decoded = decode(&path);
        std::fs::remove_file(&path).ok();
        let decoded = decoded.unwrap();
        assert_eq!(decoded.size, [BACKGROUND_SIDE as usize, BACKGROUND_SIDE as usize / 2]);
        assert_eq!(decoded.pixels[0].to_array(), [10, 20, 30, 255]);
        assert!(decode(Path::new("/nowhere/at/all.png")).is_err());
    }

    /// the settings stay inside a short screen, scrolling in it instead of running past its bottom
    #[test]
    fn the_settings_fit_a_short_screen() {
        let ctx = egui::Context::default();
        let mut menus = Menus { settings_open: true, ..Menus::default() };
        let mut settings = Settings::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(800.0, 360.0));
        for _ in 0..3 {
            let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
            ctx.run_ui(input, |ui| {
                menus.settings(ui.ctx(), &mut settings, false);
            })
            .drop_without_applying_deltas();
        }
        let window = ctx.memory(|memory| memory.area_rect(egui::Id::new("settings"))).unwrap();
        assert!(screen.contains_rect(window), "{window:?} in {screen:?}");
    }

    /// runs a frame of a panel w/ one row of a game's settings, clicking where the row starts when asked to
    fn frame_of_a_row(ctx: &egui::Context, click: bool, mut row: impl FnMut(&mut egui::Ui)) {
        let at = egui::pos2(16.0, 16.0);
        let mut events = vec![egui::Event::PointerMoved(at)];
        if click {
            let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
            events.extend([press(true), press(false)]);
        }
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(400.0, 200.0));
        let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
        let panel = egui::Frame::NONE.inner_margin(8.0);
        ctx.run_ui(input, |ui| drop(panel.show(ui, |ui| row(ui)))).drop_without_applying_deltas();
    }

    /// ticking a row gives the game the general value to start from, and unticking it has the game follow the general setting again
    #[test]
    fn ticking_a_row_gives_the_game_a_value_of_its_own() {
        let ctx = egui::Context::default();
        let mut value = None;
        for expected in [Some(3), None] {
            frame_of_a_row(&ctx, false, |ui| own(ui, &mut value, &3u32, resolution));
            frame_of_a_row(&ctx, true, |ui| own(ui, &mut value, &3u32, resolution));
            assert_eq!(value, expected);
        }
    }

    /// a game's settings only go in the file while it has some of its own, and fit a short screen like the general ones do
    #[test]
    fn a_games_settings_fit_a_short_screen() {
        let ctx = egui::Context::default();
        let mut menus = Menus { game_settings: Some((0x0004_0000_0012_3400, "A game".to_owned())), ..Menus::default() };
        let mut settings = Settings::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(800.0, 360.0));
        for _ in 0..3 {
            let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
            ctx.run_ui(input, |ui| {
                menus.game_settings(ui.ctx(), &mut settings);
            })
            .drop_without_applying_deltas();
        }
        let window = ctx.memory(|memory| memory.area_rect(egui::Id::new("game settings"))).unwrap();
        assert!(screen.contains_rect(window), "{window:?} in {screen:?}");
        assert!(settings.per_game.is_empty(), "opening the window gives the game nothing");
    }

    /// however long a message gets, its OK stays on the screen
    #[test]
    fn a_long_message_can_still_be_closed() {
        let ctx = egui::Context::default();
        let mut menus = Menus { message: Some("Recompiling a game failed, it did.\n\n".repeat(200)), ..Menus::default() };
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(800.0, 480.0));
        for _ in 0..3 {
            let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
            ctx.run_ui(input, |ui| menus.message(ui.ctx())).drop_without_applying_deltas();
        }
        let window = ctx.memory(|memory| memory.area_rect(egui::Id::new("message"))).unwrap();
        assert!(screen.contains_rect(window), "{window:?} in {screen:?}");
    }

    /// a game in line, or having its turn, doesn't get offered again
    #[test]
    fn games_already_in_line_are_left_out() {
        let wanted = |program_id, path: &str| Wanted { program_id, path: path.into() };
        let games = || [wanted(1, "a"), wanted(2, "b"), wanted(3, "c")].into_iter();
        let ids = |games: Vec<Wanted>| games.into_iter().map(|game| game.program_id).collect::<Vec<_>>();
        assert_eq!(ids(idle(games(), &[], &[])), [1, 2, 3]);
        let twice = idle([wanted(1, "a.3ds"), wanted(2, "b"), wanted(1, "a.cci")].into_iter(), &[], &[]);
        assert_eq!(twice, [wanted(1, "a.3ds"), wanted(2, "b")], "the same game twice in the folder");
        assert_eq!(ids(idle(games(), &[], &[wanted(2, "b")])), [1, 3]);
        let job = Job::at_stage(1, Stage::Compiling { done: 1, total: 2 });
        assert_eq!(ids(idle(games(), std::slice::from_ref(&job), &[wanted(2, "b")])), [3]);
        let job = Job::at_stage(1, Stage::Failed("no".to_owned()));
        assert_eq!(ids(idle(games(), std::slice::from_ref(&job), &[])), [1, 2, 3]);
    }

    #[test]
    fn keys_have_short_names() {
        assert_eq!(key_name(KeyCode::KeyX), "X");
        assert_eq!(key_name(KeyCode::ArrowUp), "Up");
        assert_eq!(key_name(KeyCode::Digit1), "1");
        assert_eq!(key_name(KeyCode::Enter), "Enter");
    }

    /// the layout editor driven through the egui frames and pointer events
    /// the app feeds it, with each frame's moves applied the way `act` does.
    struct Editor {
        ctx: egui::Context,
        menus: Menus,
        layers: Vec<(String, [f32; 4])>,
        visibility: Vec<bool>,
        canvas: [f32; 4],
        screen: egui::Rect,
        rules: Option<EditorRules>,
        controls: Vec<(usize, [Option<usize>; 4])>,
        drawn: NavigationEditorResponse,
        time: f64,
        /// the keys held, as Ctrl adding to the selection.
        modifiers: egui::Modifiers,
    }

    fn layer(name: &str, to: [f32; 4]) -> (String, [f32; 4]) {
        (name.to_owned(), to)
    }

    fn pointer_button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::default() }
    }

    /// the move an editor frame asked for, if it asked for one.
    fn editor_move(actions: &[Action]) -> Option<(usize, [f32; 4])> {
        actions.iter().find_map(|action| match action {
            Action::EditorLayer(index, to) => Some((*index, *to)),
            _ => None,
        })
    }

    impl Editor {
        fn new(layers: Vec<(String, [f32; 4])>, canvas: [f32; 4]) -> Self {
            Self::on(layers, canvas, egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1000.0, 600.0)))
        }

        fn on(layers: Vec<(String, [f32; 4])>, canvas: [f32; 4], screen: egui::Rect) -> Self {
            let visibility = vec![true; layers.len()];
            let mut editor = Editor {
                ctx: egui::Context::default(),
                menus: Menus { editor_open: true, ..Menus::default() },
                layers,
                visibility,
                canvas,
                screen,
                rules: None,
                controls: Vec::new(),
                drawn: NavigationEditorResponse::default(),
                time: 0.0,
                modifiers: egui::Modifiers::default(),
            };
            // egui hit-tests against the frame before the pointer arrives
            editor.frame(Vec::new());
            editor.frame(Vec::new());
            editor
        }

        /// the canvas point that a layer box coordinate lands on.
        fn point(&self, x: f32, y: f32) -> egui::Pos2 {
            egui::pos2(self.canvas[0] + x, self.canvas[1] + y)
        }

        fn frame(&mut self, events: Vec<egui::Event>) -> Vec<Action> {
            let events = std::iter::once(egui::Event::ModifiersChanged(self.modifiers)).chain(events).collect();
            let input = egui::RawInput { screen_rect: Some(self.screen), time: Some(self.time), events, ..Default::default() };
            self.time += 1.0 / 60.0;
            let mut actions = Vec::new();
            let menus = &mut self.menus;
            let layers = self.layers.clone();
            let visibility = self.visibility.clone();
            let canvas = self.canvas;
            let rules = self.rules;
            let controls = self.controls.clone();
            let mut drawn = NavigationEditorResponse::default();
            self.ctx
                .run_ui(input, |ui| {
                    drawn = menus.layout_editor(ui.ctx(), rules, layers.clone(), visibility.clone(), controls.clone(), [true; 4], 1.0, Some(canvas), true, &mut actions);
                })
                .drop_without_applying_deltas();
            self.drawn = drawn;
            for action in &actions {
                match action {
                    Action::EditorLayer(index, to) => {
                        if let Some(layer) = self.layers.get_mut(*index) {
                            layer.1 = *to;
                        }
                    }
                    Action::EditorLayerVisible(index, visible) => {
                        if let Some(shown) = self.visibility.get_mut(*index) {
                            *shown = *visible;
                        }
                    }
                    _ => {}
                }
            }
            actions
        }

        fn click(&mut self, pos: egui::Pos2) -> Vec<Action> {
            let mut actions = self.frame(vec![egui::Event::PointerMoved(pos)]);
            actions.extend(self.frame(vec![egui::Event::PointerMoved(pos), pointer_button(pos, true)]));
            actions.extend(self.frame(vec![pointer_button(pos, false)]));
            actions
        }

        fn press(&mut self, pos: egui::Pos2) {
            self.frame(vec![egui::Event::PointerMoved(pos)]);
            self.frame(vec![egui::Event::PointerMoved(pos), pointer_button(pos, true)]);
        }

        /// open a direction's dropdown, as a click on its button does.
        fn click_combo(&mut self, slot: usize) -> Vec<Action> {
            self.frame(Vec::new());
            let rect = self.drawn.directions.get(slot).expect("the direction's combo box").1.rect;
            let actions = self.click(rect.center());
            if self.drawn.options.is_empty() {
                self.frame(Vec::new());
            }
            assert!(!self.drawn.options.is_empty(), "the combo box's menu did not open");
            actions
        }

        /// pick an option from the dropdown open in the last frame.
        fn click_option(&mut self, value: Option<usize>) -> Vec<Action> {
            self.frame(Vec::new());
            let rect = self
                .drawn
                .options
                .iter()
                .find(|(option, _)| *option == value)
                .expect("the option in the open menu")
                .1
                .rect;
            self.click(rect.center())
        }

        /// click a layer's Visible checkbox, as a player would.
        fn click_visibility(&mut self, index: usize) -> Vec<Action> {
            self.frame(Vec::new());
            let rect = self.drawn.visibility.get(index).expect("the layer's Visible checkbox").rect;
            self.click(rect.center())
        }

        /// apply the link actions the frame asked for, as `App::act` does.
        fn apply_links(&mut self, actions: &[Action]) {
            for action in actions {
                if let Action::EditorNavigationLink(from, direction, to) = action {
                    let slot = match *direction {
                        NavigationDirection::Up => 0,
                        NavigationDirection::Down => 1,
                        NavigationDirection::Left => 2,
                        NavigationDirection::Right => 3,
                    };
                    if let Some(entry) = self.controls.iter_mut().find(|(index, _)| *index == *from) {
                        entry.1[slot] = *to;
                    }
                }
            }
        }
    }

    #[test]
    fn cauldron_canvas_drag_updates_the_live_rule() {
        use zakuro_core::aim_mouse::{AimMouse, CauldronRole, Screen};
        let mut mouse = AimMouse::default();
        mouse.cauldron_on_top = true;
        mouse.show_screen(Screen::FiendsCauldron, 10);
        let mut editor = Editor::on(mouse.editor_layers(), [1000.0, 0.0, 400.0, 240.0], egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1600.0, 600.0)));
        editor.rules = Some(EditorRules::Cauldron(mouse.cauldron_rules()));
        editor.frame(Vec::new());
        editor.frame(Vec::new());
        assert!(editor.ctx.memory(|memory| memory.area_rect(egui::Id::new("menu_editor_cauldron"))).is_some());
        let before = editor.layers[0].1;
        let start = editor.point(before[0] + 20.0, before[1] + 20.0);
        editor.press(start);
        let target = start + egui::vec2(25.0, 10.0);
        let actions = editor.frame(vec![egui::Event::PointerMoved(target)]);
        let (index, to) = editor_move(&actions).expect("dragging the picture moves it");
        assert_eq!(index, 0);
        mouse.set_editor_layer(index, to);
        let after = mouse.editor_layers()[index].1;
        assert!((after[0] - before[0] - 25.0).abs() < 0.001);
        assert!((after[1] - before[1] - 10.0).abs() < 0.001);
        mouse.set_editor_layer_visible(4, false);
        assert!(!mouse.cauldron_rules().rule(CauldronRole::Back).visible);
        mouse.show_screen(Screen::Pause, 11);
        assert!(mouse.editor_layers().is_empty(), "Pause must not expose stale Cauldron layers");
        println!("Cauldron window opened; live picture moved by (25, 10); Back hidden; Pause has no stale layers");
    }

    /// a press picks the layer under it, moving and resizing it by the
    /// distance from the press, and letting go ends the drag.
    #[test]
    fn dragging_moves_resizes_and_stops_for_the_layer_that_was_pressed() {
        let mut editor = Editor::new(
            vec![layer("left #0", [10.0, 10.0, 50.0, 20.0]), layer("right #1", [200.0, 10.0, 50.0, 20.0])],
            [500.0, 0.0, 400.0, 240.0],
        );
        let right = editor.point(210.0, 20.0);
        editor.click(right);
        println!("after clicking the right layer: selected {:?}", editor.menus.editor_layer);
        assert_eq!(editor.menus.editor_layer, Some(1));
        let start = editor.point(20.0, 20.0);
        editor.press(start);
        let target = editor.point(40.0, 20.0);
        let actions = editor.frame(vec![egui::Event::PointerMoved(target)]);
        println!("dragging the left layer {start:?} to {target:?} gave {actions:?}");
        assert_eq!(editor_move(&actions), Some((0, [30.0, 10.0, 50.0, 20.0])), "{actions:?}");
        assert_eq!(editor.menus.editor_layer, Some(0));
        // a still pointer asks for the same box, not the drag again
        let actions = editor.frame(Vec::new());
        println!("the drag held still gave {actions:?}");
        assert_eq!(editor_move(&actions), Some((0, [30.0, 10.0, 50.0, 20.0])), "a still pointer moved the layer: {actions:?}");
        // letting go ends it: moving the pointer afterwards moves nothing
        editor.frame(vec![pointer_button(target, false)]);
        let away = editor.point(120.0, 20.0);
        let actions = editor.frame(vec![egui::Event::PointerMoved(away)]);
        println!("after letting go and moving to {away:?}: {actions:?}");
        assert_eq!(editor_move(&actions), None, "the drag outlived the release: {actions:?}");
        // the bottom right corner resizes the layer instead of moving it,
        // evenly: the 50% taller drag wins over the 20% wider one
        let handle = editor.point(75.0, 25.0);
        editor.press(handle);
        let corner = editor.point(85.0, 35.0);
        let actions = editor.frame(vec![egui::Event::PointerMoved(corner)]);
        println!("resizing from {handle:?} to {corner:?} gave {actions:?}");
        assert_eq!(editor_move(&actions), Some((0, [30.0, 10.0, 75.0, 30.0])), "{actions:?}");
    }

    /// Ctrl adds a layer to the selection; dragging any member moves them
    /// all, and a member's corner scales the group about its top left.
    #[test]
    fn a_ctrl_selection_moves_and_scales_together() {
        let mut editor = Editor::new(
            vec![layer("left #0", [10.0, 10.0, 50.0, 20.0]), layer("right #1", [100.0, 10.0, 50.0, 20.0]), layer("other #2", [10.0, 100.0, 50.0, 20.0])],
            [500.0, 0.0, 400.0, 240.0],
        );
        editor.click(editor.point(20.0, 20.0));
        editor.modifiers = egui::Modifiers::COMMAND;
        editor.click(editor.point(110.0, 20.0));
        editor.modifiers = egui::Modifiers::default();
        let moves = |actions: &[Action]| {
            let mut moves: Vec<_> = actions.iter().filter_map(|action| match action {
                Action::EditorLayer(index, to) => Some((*index, *to)),
                _ => None,
            }).collect();
            moves.sort_by_key(|(index, _)| *index);
            moves
        };
        let start = editor.point(20.0, 20.0);
        editor.press(start);
        let actions = editor.frame(vec![egui::Event::PointerMoved(editor.point(40.0, 20.0))]);
        assert_eq!(moves(&actions), vec![(0, [30.0, 10.0, 50.0, 20.0]), (1, [120.0, 10.0, 50.0, 20.0])], "{actions:?}");
        editor.frame(vec![pointer_button(editor.point(40.0, 20.0), false)]);
        // the group spans 30..170 by 10..30; its right member's corner grows
        // it evenly, by the doubled height over the 10% wider width
        let handle = editor.point(168.0, 28.0);
        editor.press(handle);
        let actions = editor.frame(vec![egui::Event::PointerMoved(editor.point(182.0, 48.0))]);
        let scaled = moves(&actions);
        assert_eq!(scaled.len(), 2, "{actions:?}");
        for ((index, got), want) in scaled.iter().zip([[30.0, 10.0, 100.0, 40.0], [210.0, 10.0, 100.0, 40.0]]) {
            assert!(got.iter().zip(want).all(|(a, b)| (a - b).abs() < 0.01), "layer {index}: {got:?}, expected {want:?}");
        }
        // a plain click starts over with one layer
        editor.frame(vec![pointer_button(editor.point(182.0, 48.0), false)]);
        editor.click(editor.point(20.0, 110.0));
        assert_eq!(editor.menus.editor_group, vec![2]);
    }

    /// the layer follows the pointer from where the button went down, past
    /// the frames egui waits before it calls the press a drag.
    #[test]
    fn a_drag_moves_the_layer_from_where_the_press_began() {
        let mut editor = Editor::new(vec![layer("layer #0", [10.0, 10.0, 50.0, 20.0])], [500.0, 0.0, 400.0, 240.0]);
        let start = editor.point(20.0, 20.0);
        editor.press(start);
        // four points stays inside egui's click slop; the rest starts the drag
        let early = editor.point(24.0, 20.0);
        editor.frame(vec![egui::Event::PointerMoved(early)]);
        let target = editor.point(32.0, 20.0);
        let actions = editor.frame(vec![egui::Event::PointerMoved(target)]);
        println!("a 12 point drag from {start:?} to {target:?} gave {actions:?}");
        assert_eq!(editor_move(&actions).map(|(_, to)| to[0]), Some(22.0), "{actions:?}");
    }

    /// a layer selected in the list is dragged even where another covers
    /// it, and the arrow keys nudge the selection, Shift by ten.
    #[test]
    fn a_covered_selection_drags_and_the_arrow_keys_nudge_it() {
        let mut editor = Editor::new(
            vec![layer("below #0", [10.0, 10.0, 80.0, 40.0]), layer("above #1", [10.0, 10.0, 80.0, 40.0])],
            [500.0, 0.0, 400.0, 240.0],
        );
        // picked in the list, as its row would
        editor.menus.editor_layer = Some(0);
        editor.menus.editor_group = vec![0];
        let start = editor.point(30.0, 20.0);
        editor.press(start);
        let actions = editor.frame(vec![egui::Event::PointerMoved(editor.point(50.0, 20.0))]);
        assert_eq!(editor_move(&actions), Some((0, [30.0, 10.0, 80.0, 40.0])), "{actions:?}");
        editor.frame(vec![pointer_button(editor.point(50.0, 20.0), false)]);
        let key = |key, modifiers| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
        let actions = editor.frame(vec![key(egui::Key::ArrowRight, egui::Modifiers::default())]);
        assert_eq!(editor_move(&actions), Some((0, [31.0, 10.0, 80.0, 40.0])), "{actions:?}");
        editor.modifiers = egui::Modifiers::SHIFT;
        let actions = editor.frame(vec![key(egui::Key::ArrowUp, egui::Modifiers::SHIFT)]);
        assert_eq!(editor_move(&actions), Some((0, [31.0, 0.0, 80.0, 40.0])), "{actions:?}");
    }

    /// dragging one layer of a linked folder drags the whole folder.
    #[test]
    fn a_linked_folder_moves_together() {
        let mut editor = Editor::new(
            vec![layer("backdrop #0", [10.0, 10.0, 80.0, 40.0]), layer("emblem #1", [100.0, 10.0, 20.0, 20.0]), layer("other #2", [200.0, 10.0, 20.0, 20.0])],
            [500.0, 0.0, 400.0, 240.0],
        );
        editor.menus.editor_screen = Some(26);
        editor.menus.editor_folders = vec![EditorFolder { screen: 26, name: "Top".to_owned(), parts: vec![0, 1], linked: true, collapsed: false }];
        editor.press(editor.point(30.0, 20.0));
        let actions = editor.frame(vec![egui::Event::PointerMoved(editor.point(50.0, 20.0))]);
        editor.frame(vec![pointer_button(editor.point(50.0, 20.0), false)]);
        let moved: Vec<_> = actions.iter().filter_map(|action| match action { Action::EditorLayer(index, to) => Some((*index, *to)), _ => None }).collect();
        assert!(moved.contains(&(0, [30.0, 10.0, 80.0, 40.0])), "{moved:?}");
        assert!(moved.contains(&(1, [120.0, 10.0, 20.0, 20.0])), "{moved:?}");
        assert!(!moved.iter().any(|&(index, _)| index == 2), "{moved:?}");
        // unlinked, the same drag moves only the layer under the pointer
        editor.menus.editor_folders[0].linked = false;
        editor.click(editor.point(300.0, 200.0));
        editor.press(editor.point(50.0, 20.0));
        let actions = editor.frame(vec![egui::Event::PointerMoved(editor.point(70.0, 20.0))]);
        assert_eq!(editor_move(&actions), Some((0, [50.0, 10.0, 80.0, 40.0])), "{actions:?}");
        assert!(!actions.iter().any(|action| matches!(action, Action::EditorLayer(1, _))), "{actions:?}");
    }

    /// Ctrl+Z puts a dragged layer and a hidden one back; Ctrl+Shift+Z
    /// does them again.
    #[test]
    fn undo_and_redo_step_through_edits() {
        let mut editor = Editor::new(vec![layer("a #0", [10.0, 10.0, 80.0, 40.0]), layer("b #1", [200.0, 10.0, 20.0, 20.0])], [500.0, 0.0, 400.0, 240.0]);
        editor.press(editor.point(30.0, 20.0));
        editor.frame(vec![egui::Event::PointerMoved(editor.point(50.0, 20.0))]);
        editor.frame(vec![pointer_button(editor.point(50.0, 20.0), false)]);
        assert_eq!(editor.layers[0].1, [30.0, 10.0, 80.0, 40.0]);
        editor.click_visibility(1);
        assert!(!editor.visibility[1]);
        let key = |key, modifiers| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
        editor.modifiers = egui::Modifiers::COMMAND;
        editor.frame(vec![key(egui::Key::Z, egui::Modifiers::COMMAND)]);
        assert!(editor.visibility[1], "the last edit, hiding, is undone first");
        assert_eq!(editor.layers[0].1, [30.0, 10.0, 80.0, 40.0]);
        editor.frame(vec![key(egui::Key::Z, egui::Modifiers::COMMAND)]);
        assert_eq!(editor.layers[0].1, [10.0, 10.0, 80.0, 40.0], "then the drag");
        editor.modifiers = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
        editor.frame(vec![key(egui::Key::Z, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT)]);
        assert_eq!(editor.layers[0].1, [30.0, 10.0, 80.0, 40.0], "redo drags it again");
        assert!(editor.visibility[1]);
    }

    /// a layer index means nothing once another screen fills the list.
    #[test]
    fn a_screen_change_forgets_the_selected_layer_index() {
        let mut editor = Editor::new(
            vec![layer("left #0", [10.0, 10.0, 50.0, 20.0]), layer("right #1", [200.0, 10.0, 50.0, 20.0])],
            [500.0, 0.0, 400.0, 240.0],
        );
        let right = editor.point(210.0, 20.0);
        editor.click(right);
        assert_eq!(editor.menus.editor_layer, Some(1));
        // the game moves to a menu whose only layer sits where the left one was
        editor.layers = vec![layer("other #0", [10.0, 10.0, 50.0, 20.0])];
        editor.frame(Vec::new());
        println!("after the screen changed: selected {:?}", editor.menus.editor_layer);
        assert_eq!(editor.menus.editor_layer, None);
        let start = editor.point(20.0, 20.0);
        editor.press(start);
        let target = editor.point(40.0, 20.0);
        let actions = editor.frame(vec![egui::Event::PointerMoved(target)]);
        println!("dragging the new screen's only layer {start:?} to {target:?} gave {actions:?}");
        assert_eq!(editor_move(&actions), Some((0, [30.0, 10.0, 50.0, 20.0])), "{actions:?}");
    }

    /// F2 opens the editor over a running game, with the pause menu shut.
    #[test]
    fn the_editor_shows_over_a_game_without_the_pause_menu() {
        let ctx = egui::Context::default();
        let mut menus = Menus { editor_open: true, ..Menus::default() };
        let layers = vec![layer("layer #0", [10.0, 10.0, 50.0, 20.0])];
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1000.0, 600.0));
        for _ in 0..2 {
            let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
            ctx.run_ui(input, |ui| {
                menus.game(ui, "Game", None, None, false, false, &[], 0, None, None, layers.clone(), vec![true], Vec::new(), [true; 4], 1.0, Some([500.0, 0.0, 400.0, 240.0]), true, true);
            })
            .drop_without_applying_deltas();
        }
        let editor = ctx.memory(|memory| memory.area_rect(egui::Id::new("menu_editor")));
        let paused = ctx.memory(|memory| memory.area_rect(egui::Id::new("Paused")));
        println!("editor window {editor:?}, pause window {paused:?}");
        assert!(editor.is_some(), "the editor window is missing over a running game");
        assert!(paused.is_none(), "the pause menu must stay closed");
    }

    /// the canvas outlines layers without taking the editor window's input.
    #[test]
    fn the_editor_window_is_above_the_canvas() {
        let mut editor = Editor::new(vec![layer("layer #0", [10.0, 10.0, 50.0, 20.0])], [0.0, 0.0, 400.0, 240.0]);
        editor.frame(Vec::new());
        editor.frame(Vec::new());
        let window = editor.ctx.memory(|memory| memory.area_rect(egui::Id::new("menu_editor"))).expect("editor window");
        let point = window.min + Vec2::new(20.0, 10.0);
        let layer = editor.ctx.layer_id_at(point).expect("a layer under the window");
        println!("editor window {window:?}, layer at {point:?} is {layer:?}");
        assert_eq!(layer.id, egui::Id::new("menu_editor"), "{layer:?}");
        assert_eq!(layer.order, egui::Order::Middle, "{layer:?}");
    }

    /// a long layer list scrolls instead of running past a short screen.
    #[test]
    fn the_editor_fits_a_short_screen() {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(800.0, 360.0));
        let layers: Vec<_> = (0..60)
            .map(|index| layer(&format!("part #{index}"), [4.0 * index as f32 % 400.0, 8.0 * index as f32 % 240.0, 14.0, 8.0]))
            .collect();
        for pause in [None, Some(PauseRules::default())] {
            let mut editor = Editor::on(layers.clone(), [0.0, 0.0, 400.0, 240.0], screen);
            editor.rules = pause.map(EditorRules::Pause);
            for _ in 0..3 {
                editor.frame(Vec::new());
            }
            let (title, id) = if pause.is_some() { ("Menu editor — Pause", "menu_editor_pause") } else { ("Menu editor", "menu_editor") };
            let window = editor.ctx.memory(|memory| memory.area_rect(egui::Id::new(id))).expect(title);
            println!("{title:?} on an 800x360 screen: {window:?}");
            assert!(screen.contains_rect(window), "{title:?} {window:?} does not fit {screen:?}");
        }
    }

    /// a layer list too long for the window scrolls to the layers below.
    #[test]
    fn the_layer_list_scrolls_in_the_editor() {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(800.0, 360.0));
        let layers: Vec<_> = (0..80).map(|index| layer(&format!("part #{index}"), [0.0, 0.0, 10.0, 10.0])).collect();
        let mut editor = Editor::on(layers, [0.0, 0.0, 400.0, 240.0], screen);
        for _ in 0..3 {
            editor.frame(Vec::new());
        }
        let window = editor.ctx.memory(|memory| memory.area_rect(egui::Id::new("menu_editor"))).expect("editor window");
        // a little way into the first row's name, past its Visible checkbox
        let row = editor.drawn.visibility[0].rect;
        let list = egui::pos2(row.max.x + 25.0, row.center().y);
        editor.click(list);
        let before = editor.menus.editor_layer.expect("a layer above the fold");
        for _ in 0..16 {
            editor.frame(vec![
                egui::Event::PointerMoved(list),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -120.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: egui::Modifiers::default(),
                },
            ]);
        }
        editor.click(list);
        let after = editor.menus.editor_layer.expect("a layer after scrolling");
        println!("layer at {list:?} in {window:?} was #{before}, after scrolling is #{after}");
        assert!(after > before, "the layer list did not scroll: #{before} then #{after}");
    }

    /// a pointer selects a Control on the canvas, picks a direction's target
    /// and Native default through the real combo boxes, and the action names
    /// the selected Control and direction each time. A decorative layer
    /// offers no dead links.
    #[test]
    fn navigation_dropdown_sets_direction_targets_and_restores_native() {
        let layers = vec![
            layer("Main Solo #0", [10.0, 10.0, 60.0, 20.0]),
            layer("Vault2 #1", [200.0, 10.0, 60.0, 20.0]),
            layer("hint #2", [10.0, 120.0, 60.0, 20.0]),
        ];
        let mut editor = Editor::new(layers, [500.0, 0.0, 400.0, 240.0]);
        editor.controls = vec![(0, [None; 4]), (1, [None; 4])];
        // the editor selects the layer the canvas click is over.
        editor.click(editor.point(20.0, 20.0));
        assert_eq!(editor.menus.editor_layer, Some(0));
        assert_eq!(editor.drawn.directions.len(), 4, "the selected Control has four direction menus");

        // Down on Main Solo points at Vault2.
        let actions = editor.click_combo(1);
        assert!(actions.is_empty(), "opening a direction menu chose a link");
        let actions = editor.click_option(Some(1));
        assert_eq!(
            actions,
            vec![Action::EditorNavigationLink(0, NavigationDirection::Down, Some(1))],
            "the chosen target did not name Main Solo's Down link"
        );
        editor.apply_links(&actions);
        // the snapshot now holds the link, so reopening the menu changes nothing.
        assert!(editor.frame(Vec::new()).is_empty(), "applying a link emitted another action");
        let actions = editor.click_combo(1);
        assert!(actions.is_empty(), "reopening a restored link emitted a stale action");
        let actions = editor.click_option(None);
        assert_eq!(
            actions,
            vec![Action::EditorNavigationLink(0, NavigationDirection::Down, None)],
            "Native default did not remove only Main Solo's Down link"
        );
        editor.apply_links(&actions);

        // the other Control links Up to itself: the action names it, not Main Solo.
        editor.click(editor.point(210.0, 20.0));
        assert_eq!(editor.menus.editor_layer, Some(1));
        let actions = editor.click_combo(0);
        assert!(actions.is_empty(), "opening a direction menu chose a link");
        let actions = editor.click_option(Some(1));
        assert_eq!(
            actions,
            vec![Action::EditorNavigationLink(1, NavigationDirection::Up, Some(1))],
            "the self-link named the wrong Control or direction"
        );
        editor.apply_links(&actions);

        // the decorative hint is not offered links at all.
        editor.menus.editor_layer = Some(2);
        assert!(editor.frame(Vec::new()).is_empty(), "a decorative hint emitted a link");
        assert!(editor.drawn.directions.is_empty(), "a decorative hint was offered direction menus");
        assert!(editor.drawn.options.is_empty(), "a decorative hint was offered link targets");
    }

    /// a layer's Visible checkbox hides it from the canvas while keeping it
    /// listed, and showing it again restores the same box and pointer.
    #[test]
    fn a_hidden_layer_stays_listed_but_takes_no_canvas_pointer() {
        let mut editor = Editor::new(
            vec![layer("left #0", [10.0, 10.0, 50.0, 20.0]), layer("right #1", [200.0, 10.0, 50.0, 20.0])],
            [500.0, 0.0, 400.0, 240.0],
        );
        editor.menus.editor_layer = Some(1);
        let actions = editor.click_visibility(0);
        assert_eq!(actions, vec![Action::EditorLayerVisible(0, false)], "the checkbox did not hide the layer: {actions:?}");
        assert_eq!(editor.menus.editor_layer, Some(0), "the toggle did not select the layer it hid");
        assert_eq!(editor.visibility, vec![false, true]);
        assert_eq!(editor.layers.len(), 2, "a hidden layer left the list");
        assert_eq!(editor.drawn.visibility.len(), 2, "a hidden layer lost its checkbox");
        // the list still selects it, for showing it again.
        let checkbox = editor.drawn.visibility[0].rect;
        editor.click(egui::pos2(checkbox.max.x + 30.0, checkbox.center().y));
        assert_eq!(editor.menus.editor_layer, Some(0), "a hidden layer is not selectable in the list");

        // a click over the hidden box selects nothing on the canvas.
        editor.menus.editor_layer = Some(1);
        editor.click(editor.point(20.0, 20.0));
        assert_eq!(editor.menus.editor_layer, None, "a hidden layer took a canvas click");
        // and a press over it moves nothing, though the list can still
        // select it to show it again.
        editor.menus.editor_layer = Some(0);
        editor.press(editor.point(20.0, 20.0));
        let target = editor.point(60.0, 30.0);
        let actions = editor.frame(vec![egui::Event::PointerMoved(target)]);
        editor.frame(vec![pointer_button(target, false)]);
        assert_eq!(editor_move(&actions), None, "a hidden layer moved: {actions:?}");
        assert_eq!(editor.layers[0].1, [10.0, 10.0, 50.0, 20.0]);

        // showing it again keeps its box and makes it draggable.
        let actions = editor.click_visibility(0);
        assert_eq!(actions, vec![Action::EditorLayerVisible(0, true)], "the checkbox did not show the layer: {actions:?}");
        assert!(editor.visibility[0]);
        editor.click(editor.point(20.0, 20.0));
        assert_eq!(editor.menus.editor_layer, Some(0));
        editor.press(editor.point(20.0, 20.0));
        let actions = editor.frame(vec![egui::Event::PointerMoved(editor.point(40.0, 20.0))]);
        assert_eq!(editor_move(&actions), Some((0, [30.0, 10.0, 50.0, 20.0])), "{actions:?}");
    }

    /// hiding the layer over another lets the one under the pointer take
    /// the press, and only that one moves.
    #[test]
    fn hiding_an_obscuring_layer_lets_the_one_below_take_the_pointer() {
        let mut editor = Editor::new(
            vec![layer("below #0", [10.0, 10.0, 80.0, 40.0]), layer("above #1", [10.0, 10.0, 80.0, 40.0])],
            [500.0, 0.0, 400.0, 240.0],
        );
        editor.click(editor.point(20.0, 20.0));
        assert_eq!(editor.menus.editor_layer, Some(1), "the upper layer should take the press first");
        let actions = editor.click_visibility(1);
        assert_eq!(actions, vec![Action::EditorLayerVisible(1, false)], "{actions:?}");

        editor.click(editor.point(20.0, 20.0));
        assert_eq!(editor.menus.editor_layer, Some(0), "the hidden upper layer still took the press");
        editor.press(editor.point(20.0, 20.0));
        let actions = editor.frame(vec![egui::Event::PointerMoved(editor.point(40.0, 20.0))]);
        assert_eq!(editor_move(&actions), Some((0, [30.0, 10.0, 80.0, 40.0])), "{actions:?}");
        assert_eq!(editor.layers[1].1, [10.0, 10.0, 80.0, 40.0], "the hidden layer moved");
    }

}

/// one rule's visibility, source and target boxes, its anchor and its
/// scale mode.
fn rule_editor(ui: &mut egui::Ui, name: &str, rule: &mut PlacementRule) -> bool {
    let mut changed = false;
    ui.collapsing(name, |ui| {
        changed |= ui.checkbox(&mut rule.visible, "Visible").changed();
        changed |= vec4(ui, "from", &mut rule.from);
        changed |= vec4(ui, "to", &mut rule.to);
        ui.horizontal(|ui| {
            ui.label("anchor");
            changed |= ui.add(egui::DragValue::new(&mut rule.anchor).speed(0.01)).changed();
        });
        egui::ComboBox::from_id_salt(name).selected_text(format!("{:?}", rule.scale)).show_ui(ui, |ui| {
            changed |= ui.selectable_value(&mut rule.scale, RuleScale::Fixed, "Fixed").changed();
            changed |= ui.selectable_value(&mut rule.scale, RuleScale::Narrow, "Narrow").changed();
            changed |= ui.selectable_value(&mut rule.scale, RuleScale::Stretch, "Stretch").changed();
        });
    });
    changed
}

fn vec4(ui: &mut egui::Ui, label: &str, values: &mut [f32; 4]) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        for value in values.iter_mut() {
            changed |= ui.add(egui::DragValue::new(value).speed(1.0)).changed();
        }
    });
    changed
}

/// each layer's name and whether it is drawn, with a Visible checkbox that
/// hides it and keeps it listed for showing again. Ctrl or Shift on a name
/// adds it to the selection or takes it out. On a Scripted Menu, layers can
/// be put in folders: a folder lists its layers under it where its first
/// one would be, selects them all, shows or hides them all, and, linked,
/// keeps them selected together. Undo and Redo step through the edits.
#[allow(clippy::too_many_arguments)]
fn layer_list(
    ui: &mut egui::Ui,
    layers: &[(String, [f32; 4])],
    visibility: &[bool],
    selected: &mut Option<usize>,
    group: &mut Vec<usize>,
    folders: &mut Vec<EditorFolder>,
    screen: Option<usize>,
    renaming: &mut Option<usize>,
    [can_undo, can_redo]: [bool; 2],
    actions: &mut Vec<Action>,
) -> LayerListResponse {
    let mut response = LayerListResponse::default();
    if layers.is_empty() { return response; }
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Layers").strong());
        response.undo = ui.add_enabled(can_undo, egui::Button::new("Undo").small()).on_hover_text("Step back one edit (Ctrl+Z).").clicked();
        response.redo = ui.add_enabled(can_redo, egui::Button::new("Redo").small()).on_hover_text("Step forward again (Ctrl+Shift+Z or Ctrl+Y).").clicked();
        if let Some(screen) = screen {
            let new = ui.add_enabled(!group.is_empty(), egui::Button::new("New folder").small())
                .on_hover_text("Put the selected layers in a new folder.");
            if new.clicked() {
                let mut parts = group.clone();
                parts.sort_unstable();
                for folder in folders.iter_mut() { folder.parts.retain(|part| !parts.contains(part)); }
                folders.retain(|folder| !folder.parts.is_empty());
                let name = (1..).map(|number| format!("Folder {number}")).find(|name| !folders.iter().any(|folder| &folder.name == name)).unwrap_or_default();
                folders.push(EditorFolder { screen, name, parts, linked: false, collapsed: false });
            }
        }
    });
    ui.label(RichText::new("Ctrl or Shift selects several to move or scale together. A linked folder's layers always move together.").small().weak());
    let adding = ui.input(|input| input.modifiers.command || input.modifiers.shift);
    let shown = |index: usize| visibility.get(index).copied().unwrap_or(true);
    let mut boxes: Vec<Option<egui::Response>> = vec![None; layers.len()];
    let mut listed = vec![false; folders.len()];
    // what a folder's buttons asked for, done after the list is drawn
    let (mut take_out, mut ungroup, mut add_to) = (None, None, None);
    for index in 0..layers.len() {
        let Some(at) = folders.iter().position(|folder| folder.parts.contains(&index)) else {
            layer_row(ui, index, &layers[index].0, shown(index), false, adding, selected, group, actions, &mut boxes, &mut response.edited);
            continue;
        };
        if listed[at] { continue; }
        listed[at] = true;
        let mut members = folders[at].parts.clone();
        members.sort_unstable();
        ui.horizontal(|ui| {
            let folder = &mut folders[at];
            let arrow = if folder.collapsed { "\u{25B6}" } else { "\u{25BC}" };
            if ui.small_button(arrow).on_hover_text(if folder.collapsed { "Show this folder's layers." } else { "Fold this folder's layers away." }).clicked() {
                folder.collapsed = !folder.collapsed;
            }
            let all_shown = members.iter().all(|&member| shown(member));
            let mut show = all_shown;
            if ui.checkbox(&mut show, "").on_hover_text("Draw and use every layer in this folder.").changed() {
                for &member in members.iter().filter(|&&member| shown(member) != show) {
                    actions.push(Action::EditorLayerVisible(member, show));
                }
                *group = members.clone();
                *selected = members.first().copied();
                response.edited = true;
            }
            if *renaming == Some(at) {
                let edit = ui.add(egui::TextEdit::singleline(&mut folder.name).desired_width(120.0));
                if edit.lost_focus() {
                    *renaming = None;
                    if folder.name.trim().is_empty() { folder.name = "Folder".to_owned(); }
                } else if !edit.has_focus() {
                    edit.request_focus();
                }
            } else {
                let picked = members.iter().all(|member| group.contains(member));
                let name = ui.selectable_label(picked, RichText::new(&folder.name).strong())
                    .on_hover_text("Click to select this folder's layers; double-click to rename it.");
                if name.double_clicked() {
                    *renaming = Some(at);
                } else if name.clicked() {
                    if adding && picked {
                        group.retain(|member| !members.contains(member));
                        *selected = group.last().copied();
                    } else {
                        if !adding { group.clear(); }
                        for &member in &members {
                            if !group.contains(&member) { group.push(member); }
                        }
                        *selected = members.first().copied();
                    }
                }
            }
            ui.checkbox(&mut folder.linked, "Linked")
                .on_hover_text("Picking any layer in this folder picks them all, so they move and scale together.");
            let outside = group.iter().any(|member| !members.contains(member));
            if ui.add_enabled(outside, egui::Button::new("Add").small()).on_hover_text("Move the selected layers into this folder.").clicked() {
                add_to = Some(at);
            }
            if ui.small_button("Ungroup").on_hover_text("Remove the folder and keep its layers.").clicked() {
                ungroup = Some(at);
            }
        });
        if !folders[at].collapsed {
            for &member in &members {
                if layer_row(ui, member, &layers[member].0, shown(member), true, adding, selected, group, actions, &mut boxes, &mut response.edited) {
                    take_out = Some(member);
                }
            }
        }
    }
    if let Some(at) = add_to {
        let joining: Vec<usize> = group.clone();
        for folder in folders.iter_mut() { folder.parts.retain(|part| !joining.contains(part)); }
        for part in joining {
            if !folders[at].parts.contains(&part) { folders[at].parts.push(part); }
        }
    }
    if let Some(member) = take_out {
        for folder in folders.iter_mut() { folder.parts.retain(|&part| part != member); }
    }
    if let Some(at) = ungroup {
        folders.remove(at);
        *renaming = None;
    }
    if add_to.is_some() || take_out.is_some() {
        // a folder left empty goes; folders after it move up a place
        let before = folders.len();
        folders.retain(|folder| !folder.parts.is_empty());
        if folders.len() != before { *renaming = None; }
    }
    response.boxes = boxes.into_iter().flatten().collect();
    response
}

/// one layer's row: its Visible checkbox and its name, and, in a folder,
/// indented with a button taking it out, which returns true when pressed.
#[allow(clippy::too_many_arguments)]
fn layer_row(
    ui: &mut egui::Ui,
    index: usize,
    name: &str,
    visible: bool,
    nested: bool,
    adding: bool,
    selected: &mut Option<usize>,
    group: &mut Vec<usize>,
    actions: &mut Vec<Action>,
    boxes: &mut [Option<egui::Response>],
    edited: &mut bool,
) -> bool {
    let mut take_out = false;
    ui.horizontal(|ui| {
        if nested { ui.add_space(18.0); }
        let mut visible = visible;
        let response = ui.checkbox(&mut visible, "").on_hover_text("Draw and use this layer in the game.");
        if response.changed() {
            // the toggled layer is the one the editor is about now, so
            // it can be shown again or checked on the canvas deliberately.
            *selected = Some(index);
            *group = vec![index];
            actions.push(Action::EditorLayerVisible(index, visible));
            *edited = true;
        }
        boxes[index] = Some(response);
        if ui.selectable_label(group.contains(&index), name).clicked() {
            if !adding {
                *group = vec![index];
                *selected = Some(index);
            } else if group.contains(&index) {
                group.retain(|&member| member != index);
                *selected = group.last().copied();
            } else {
                group.push(index);
                *selected = Some(index);
            }
        }
        if nested && ui.small_button("Take out").on_hover_text("Take this layer out of its folder.").clicked() {
            take_out = true;
        }
    });
    take_out
}

/// the boxes a drag asks for, by `by` layer units from where each began:
/// moved together, or scaled evenly about the group's top left corner, so
/// each layer keeps its aspect ratio and a group its arrangement.
fn dragged(start: &[(usize, [f32; 4])], by: [f32; 2], resizing: bool) -> Vec<(usize, [f32; 4])> {
    if !resizing {
        return start.iter().map(|&(index, to)| (index, [to[0] + by[0], to[1] + by[1], to[2], to[3]])).collect();
    }
    let Some(&(_, first)) = start.first() else { return Vec::new() };
    let bounds = start.iter().fold(first, |b, &(_, to)| {
        let (x, y) = (b[0].min(to[0]), b[1].min(to[1]));
        [x, y, (b[0] + b[2]).max(to[0] + to[2]) - x, (b[1] + b[3]).max(to[1] + to[3]) - y]
    });
    let (width, height) = (bounds[2].max(1.0), bounds[3].max(1.0));
    // the axis the pointer moved further along, for its size, sets the scale
    let (across, down) = ((width + by[0]) / width, (height + by[1]) / height);
    let scale = if (across - 1.0).abs() >= (down - 1.0).abs() { across } else { down };
    // no layer shrinks below one unit on either side
    let least = start.iter().fold(0.0f32, |least, &(_, to)| least.max(1.0 / to[2].max(1.0)).max(1.0 / to[3].max(1.0)));
    let scale = scale.max(least);
    start.iter().map(|&(index, to)| (index, [
        bounds[0] + (to[0] - bounds[0]) * scale,
        bounds[1] + (to[1] - bounds[1]) * scale,
        to[2] * scale,
        to[3] * scale,
    ])).collect()
}

const NAVIGATION_DIRECTIONS: [(NavigationDirection, &str); 4] = [
    (NavigationDirection::Up, "Up"),
    (NavigationDirection::Down, "Down"),
    (NavigationDirection::Left, "Left"),
    (NavigationDirection::Right, "Right"),
];

/// the widgets the navigation section drew, for the interaction test to
/// press them as a player would.
#[derive(Default)]
struct NavigationEditorResponse {
    /// each direction's combo box button, in Up Down Left Right order.
    #[cfg(test)]
    directions: Vec<(NavigationDirection, egui::Response)>,
    /// the open combo's options, as the value each would pick.
    #[cfg(test)]
    options: Vec<(Option<usize>, egui::Response)>,
    /// each layer's Visible checkbox, in the layer list's order.
    #[cfg(test)]
    visibility: Vec<egui::Response>,
}

/// the selected Control's explicit Up, Down, Left and Right links, each
/// either the game's native behavior or another active Control.
fn navigation_editor(
    ui: &mut egui::Ui,
    layers: &[(String, [f32; 4])],
    controls: &[(usize, [Option<usize>; 4])],
    directions: [bool; 4],
    selected: Option<usize>,
    actions: &mut Vec<Action>,
) -> NavigationEditorResponse {
    #[cfg(test)]
    let mut drawn = NavigationEditorResponse::default();
    #[cfg(not(test))]
    let drawn = NavigationEditorResponse::default();
    ui.separator();
    ui.label(RichText::new("Navigation").strong());
    let Some(selected) = selected.filter(|index| *index < layers.len()) else {
        ui.label(RichText::new("Select a Control on the canvas to set its Up, Down, Left and Right links.").weak());
        return drawn;
    };
    let Some((_, destinations)) = controls.iter().find(|(index, _)| *index == selected) else {
        ui.label(
            RichText::new("This layer has no editable native navigation binding. Select an active Control; background layers and inactive Controls retain native navigation.")
                .weak(),
        );
        return drawn;
    };
    ui.label(RichText::new(layers[selected].0.clone()).small().weak());
    for (slot, (direction, label)) in NAVIGATION_DIRECTIONS.iter().enumerate() {
        if !directions[slot] {
            ui.label(RichText::new(format!("{label}: native direction unavailable")).small().weak());
            continue;
        }
        let current = destinations[slot];
        let mut destination = current;
        let text = match current {
            None => "Native default".to_owned(),
            Some(to) if to == selected => "Stay on this Control (self-link)".to_owned(),
            Some(to) => layers.get(to).map_or_else(|| format!("Control #{to}"), |(name, _)| name.clone()),
        };
        #[cfg(test)]
        let mut options = Vec::new();
        let combo = ui.horizontal(|ui| {
            ui.label(*label);
            egui::ComboBox::from_id_salt(("editor_navigation", selected, slot))
                .width(190.0)
                .selected_text(text)
                .show_ui(ui, |ui| {
                    #[cfg(test)]
                    options.push((None, ui.selectable_value(&mut destination, None, "Native default")));
                    #[cfg(not(test))]
                    ui.selectable_value(&mut destination, None, "Native default");
                    for (index, (name, _)) in layers.iter().enumerate() {
                        if !controls.iter().any(|(control, _)| *control == index) {
                            continue;
                        }
                        let text = if index == selected { "Stay on this Control (self-link)".to_owned() } else { name.clone() };
                        #[cfg(test)]
                        options.push((Some(index), ui.selectable_value(&mut destination, Some(index), text)));
                        #[cfg(not(test))]
                        ui.selectable_value(&mut destination, Some(index), text);
                    }
                })
        });
        #[cfg(test)]
        {
            drawn.directions.push((*direction, combo.inner.response));
            drawn.options.extend(options);
        }
        #[cfg(not(test))]
        let _ = combo;
        if destination != current {
            actions.push(Action::EditorNavigationLink(selected, *direction, destination));
        }
    }
    ui.label(
        RichText::new(
            "Links are explicit and replace only that direction; Native default follows the game's own behavior. Moving or resizing layers does not rewrite them. F2 closes the editor to test the game's keyboard, D-pad, stick and A.",
        )
        .weak()
        .small(),
    );
    drawn
}

fn export_layers(layers: &[(String, [f32; 4])], visibility: &[bool]) -> std::io::Result<()> {
    let mut text = String::from("// Scripted Menu layers: name, target x y width height on the top screen, visible.\n[\n");
    for (index, (name, to)) in layers.iter().enumerate() {
        let visible = visibility.get(index).copied().unwrap_or(true);
        text.push_str(&format!("    ({name:?}, {:?}, {visible}),\n", to));
    }
    text.push_str("]\n\n// Change request\n");
    for (index, (name, to)) in layers.iter().enumerate() {
        if visibility.get(index).copied().unwrap_or(true) {
            text.push_str(&format!("Move the {name} layer: target now {:?}.\n", to));
        } else {
            text.push_str(&format!("Hide the {name} layer: it stays in the editor list to show again.\n"));
        }
    }
    text.push_str("Acceptance: on a Scripted Menu capture the named layers sit at the exported boxes, hit-testing follows the same boxes, and hidden layers are neither drawn nor hit.\n");
    std::fs::write("editor-layers-export.txt", text)
}
