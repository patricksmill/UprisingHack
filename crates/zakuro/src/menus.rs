//! the library, the menu over a game and the settings, drawn with egui.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use egui::{Align, Color32, Layout, RichText, Vec2};
use winit::keyboard::KeyCode;
use zakuro_core::aim_mouse::{CauldronRules, NavigationDirection, PauseRules, PlacementRule, RuleScale};
use zakuro_core::services::keyboard::Request;

use crate::library::{Library, ICON_SIZE};
use crate::recompile::{Job, Stage};
use crate::gamepad::button_name;
use crate::settings::{CameraSource, Filter, Keys, Language, PadButtons, Renderer, Screens, Settings, Stereo3d, TopShape};

/// what the user asked for.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Play(PathBuf),
    /// recompile the library's game at this index.
    Recompile(usize),
    /// download a compiler, then recompile the library's game at this
    /// index with it.
    DownloadCompiler(usize),
    /// show the mods folder of the game with this program id.
    Mods(u64),
    CancelRecompile(u64),
    ChooseFolder,
    ChooseBackground,
    /// pick the picture the cameras see.
    ChooseCameraPicture,
    /// save the game's state to the slot chosen, or load it from there.
    SaveState,
    LoadState,
    /// open a room and join it, join another's, or leave the one joined.
    HostRoom,
    JoinRoom,
    LeaveRoom,
    /// take part in the Download Play offer at this place in the list, or
    /// leave the one taken part in.
    DlpJoin(usize),
    DlpLeave,
    Rescan,
    Resume,
    Reset,
    Library,
    Fullscreen,
    /// copy what a report on the game wants.
    CopyReport,
    Quit,
    /// the settings changed, to save and apply.
    Settings,
    /// close the game's keyboard with this text and the button pressed.
    Keyboard(String, usize),
    /// turn the running game's cheat at this index on or off.
    Cheat(usize, bool),
    /// add a cheat, its name and its code, to the running game.
    AddCheat(String, String),
    /// read the running game's cheats from their file again.
    ReloadCheats,
    /// show the cheats folder.
    CheatsFolder,
    /// the pause layout rules as the editor left them.
    PauseRules(PauseRules),
    /// the Fiend's Cauldron's rules, edited.
    CauldronRules(CauldronRules),
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
    /// the menu over a running game.
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
    /// the cheats window, and the cheat being written in it.
    pub cheats_open: bool,
    cheat_name: String,
    cheat_code: String,
    pub settings_open: bool,
    /// the binding waiting for a key, by its place in Keys::all_mut.
    pub rebinding: Option<usize>,
    /// the controller binding waiting for a button, by its place in
    /// PadButtons::all_mut.
    pub rebinding_pad: Option<usize>,
    /// something to tell the user, until they close it.
    pub message: Option<String>,
    /// something to tell the user for a moment, over the game, and since
    /// when.
    pub toast: Option<(String, std::time::Instant)>,
    /// the slot states are saved to and loaded from, 1 to 9.
    pub slot: u8,
    /// the multiplayer window, and the password typed in it.
    pub multiplayer_open: bool,
    pub room_password: String,
    /// the icons of the games offered for Download Play.
    dlp_icons: HashMap<u64, egui::TextureHandle>,
    /// a report on the game, to copy from the message it goes with, and
    /// from no other.
    pub report: Option<(String, String)>,
    /// what is being typed into the game's keyboard, and what it asked for.
    typing: Option<(Request, String)>,
    /// the library's game to recompile once the player agrees to download a
    /// compiler for it.
    pub compiler_offer: Option<usize>,
    icons: HashMap<PathBuf, egui::TextureHandle>,
    background: Background,
}

/// the room the player is in, as the multiplayer window shows it.
pub struct RoomView {
    pub name: String,
    /// each member's nickname and the game they play.
    pub members: Vec<(String, String)>,
    /// the port of the room this program holds open, when it is its own.
    pub port: Option<u16>,
    /// Download Play, while no game runs.
    pub download_play: Option<DlpView>,
}

/// the Download Play offers in the room, and where the guest is.
pub struct DlpView {
    pub offers: Vec<OfferView>,
    /// what the guest is doing, and how far along, while it takes part.
    pub doing: Option<(String, Option<f32>)>,
}

pub struct OfferView {
    pub child: u64,
    pub title: String,
    pub host: String,
    /// RGBA, ICON_SIZE square.
    pub icon: Option<Vec<u8>>,
    pub guests: u8,
    pub most: u8,
    /// why the player can't take part, when they can't.
    pub blocked: Option<String>,
}

/// the picture behind the library, decoded away from the interface.
#[derive(Default)]
struct Background {
    /// the file shown or being read.
    path: Option<PathBuf>,
    texture: Option<egui::TextureHandle>,
    loading: Option<Receiver<Result<egui::ColorImage, String>>>,
}

/// the longest side a background is kept at, bigger ones are scaled down.
const BACKGROUND_SIDE: u32 = 2560;

impl Background {
    /// the texture for path, starting to read it when it changed, none until
    /// it is ready or when there is no picture.
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

/// the part of a picture that fills an area without stretching it, cutting
/// off what sticks out on either side.
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

impl Menus {
    /// the library, filling the window.
    pub fn library(&mut self, ui: &mut egui::Ui, library: &Library, settings: &Settings, jobs: &[Job]) -> Vec<Action> {
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
                for (index, game) in library.games.iter().enumerate() {
                    let job = jobs.iter().find(|job| job.program_id == game.program_id);
                    // over a picture, each game gets a backing to keep it
                    // readable
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
                                    if game.recompiled {
                                        ui.label(RichText::new("recompiled").color(Color32::from_rgb(110, 200, 120)));
                                    }
                                    if game.modded {
                                        ui.label(RichText::new("mods").color(Color32::from_rgb(120, 170, 230)));
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
                                    _ => {
                                        let label = if game.recompiled { "Recompile again" } else { "Recompile" };
                                        if ui.button(label).on_hover_text("Turn the game's code into native code with 3dsrecomp, which makes it run faster. It takes around ten minutes and needs a C compiler, gcc or clang, or Zig, which Zakuro offers to download when the computer has neither.").clicked() {
                                            actions.push(Action::Recompile(index));
                                        }
                                        if let Some(Stage::Failed(error)) = job.map(Job::stage) {
                                            ui.label(RichText::new(format!("failed, {error}")).color(Color32::LIGHT_RED));
                                        }
                                    }
                                }
                                if ui.button("Mods").on_hover_text("Open the game's mods folder. Files put in its romfs folder replace the game's, the way Luma3DS and Citra take mods, and a texture pack made for Citra or Azahar goes in its textures folder.").clicked() {
                                    actions.push(Action::Mods(game.program_id));
                                }
                            });
                        });
                    });
                }
            });
        });
        actions
    }

    /// what goes over a running game, the menu when it is open and the
    /// frame rate when asked for.
    #[allow(clippy::too_many_arguments)]
    pub fn game(
        &mut self,
        ui: &mut egui::Ui,
        name: &str,
        fps: Option<FpsReadout>,
        recompiled: bool,
        fast: bool,
        jobs: &[Job],
        saved: Option<&str>,
        pause_rules: Option<PauseRules>,
        cauldron_rules: Option<CauldronRules>,
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
            // clicks go through to the screen under it
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
                    for job in jobs {
                        ui.separator();
                        ui.label(&job.name);
                        progress(ui, job);
                    }
                });
        }
        self.layout_editor(&ctx, pause_rules, cauldron_rules, editor_layers, editor_visibility, navigation_controls, navigation_directions, editor_widen, canvas, editor_open, &mut actions);
        actions
    }

    /// the dev-only layout editor: the edited Screen's layers on a canvas,
    /// and the Pause or Fiend's Cauldron rules, live, with reset and export.
    fn layout_editor(
        &mut self,
        ctx: &egui::Context,
        rules: Option<PauseRules>,
        cauldron: Option<CauldronRules>,
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
                                group.retain(|&member| member != index);
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
                        if selected.is_none() { resizing = false; }
                    }
                }
                if response.drag_started() {
                    // where the button went down, not where the pointer is now:
                    // a corner drag leaves the layer before egui calls it a drag
                    let point = ctx.input(|input| input.pointer.press_origin());
                    let hit = point.and_then(under);
                    // dragging a member moves the whole group
                    match hit {
                        Some(index) if group.contains(&index) => {}
                        Some(index) if adding => group.push(index),
                        _ => group = hit.into_iter().collect(),
                    }
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
            });
            self.editor_layer = selected;
            self.editor_group = group;
            self.editor_resizing = resizing;
            self.editor_drag = drag;
            actions.extend(moved.into_iter().map(|(index, to)| Action::EditorLayer(index, to)));
        }
        if let Some(cauldron) = cauldron {
            self.cauldron_editor(ctx, cauldron, &layers, &visibility, actions);
            return NavigationEditorResponse::default();
        }
        let Some(mut rules) = rules else {
            let mut window_open = true;
            let mut save = false;
            let mut export = false;
            let mut navigation = NavigationEditorResponse::default();
            #[cfg(test)]
            let mut visible_boxes = Vec::new();
            let room = (ctx.content_rect().height() - 150.0).clamp(120.0, 420.0);
            egui::Window::new("Menu editor").id(egui::Id::new("menu_editor")).open(&mut window_open).default_width(380.0).show(ctx, |ui| {
                ui.label(if layers.is_empty() { "Open a Scripted Menu or the Pause menu to edit its layout." } else { "Select a layer on the canvas to move or resize it." });
                egui::ScrollArea::vertical().max_height(room).show(ui, |ui| {
                    #[cfg(test)]
                    {
                        visible_boxes = layer_list(ui, &layers, &visibility, &mut self.editor_layer, &mut self.editor_group, actions);
                    }
                    #[cfg(not(test))]
                    {
                        layer_list(ui, &layers, &visibility, &mut self.editor_layer, &mut self.editor_group, actions);
                    }
                    navigation = navigation_editor(ui, &layers, &navigation_controls, navigation_directions, self.editor_layer, actions);
                });
                if !layers.is_empty() {
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Save layout").on_hover_text("Write the moved and hidden layers and navigation links to menu-layout.toml.").clicked() {
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
                navigation.visibility = visible_boxes;
            }
            if export {
                self.message = Some(match export_layers(&layers, &visibility) {
                    Ok(()) => "Wrote editor-layers-export.txt".to_owned(),
                    Err(error) => format!("Could not write editor-layers-export.txt: {error}"),
                });
            }
            if save { actions.push(Action::SaveLayout); }
            return navigation;
        };
        let mut window_open = true;
        let mut changed = false;
        let mut save = false;
        let mut message = None;
        let mut navigation = NavigationEditorResponse::default();
        #[cfg(test)]
        let mut visible_boxes = Vec::new();
        let room = (ctx.content_rect().height() - 170.0).clamp(120.0, 420.0);
        egui::Window::new("Menu editor — Pause").id(egui::Id::new("menu_editor_pause")).open(&mut window_open).default_width(420.0).show(ctx, |ui| {
            ui.label(RichText::new("Edits apply live. F2 closes and reopens this window.").weak());
            egui::ScrollArea::vertical().max_height(room).show(ui, |ui| {
                changed |= rule_editor(ui, "span", &mut rules.span);
                changed |= rule_editor(ui, "center", &mut rules.center);
                changed |= rule_editor(ui, "left", &mut rules.left);
                changed |= rule_editor(ui, "right", &mut rules.right);
                changed |= rule_editor(ui, "corner", &mut rules.corner);
                #[cfg(test)]
                {
                    visible_boxes = layer_list(ui, &layers, &visibility, &mut self.editor_layer, &mut self.editor_group, actions);
                }
                #[cfg(not(test))]
                {
                    layer_list(ui, &layers, &visibility, &mut self.editor_layer, &mut self.editor_group, actions);
                }
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
                if ui.button("Save layout").on_hover_text("Write the pause rules, the moved and hidden layers and navigation links to menu-layout.toml.").clicked() {
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
            navigation.visibility = visible_boxes;
        }
        if changed { actions.push(Action::PauseRules(rules)); }
        if save { actions.push(Action::SaveLayout); }
        if let Some(message) = message { self.message = Some(message); }
        navigation
    }

    /// the Fiend's Cauldron's window: its seven parts listed with Visible
    /// checkboxes, moved and resized on the canvas, then saved, reset or
    /// exported as Rust and a change request.
    fn cauldron_editor(&mut self, ctx: &egui::Context, rules: CauldronRules, layers: &[(String, [f32; 4])], visibility: &[bool], actions: &mut Vec<Action>) {
        let mut window_open = true;
        let (mut save, mut reset) = (false, false);
        let mut message = None;
        let room = (ctx.content_rect().height() - 170.0).clamp(120.0, 420.0);
        egui::Window::new("Menu editor — Fiend's Cauldron").id(egui::Id::new("menu_editor_cauldron")).open(&mut window_open).default_width(380.0).show(ctx, |ui| {
            ui.label(RichText::new("Drag a part on the canvas to move it; its corner handle resizes it. Edits apply live.").weak());
            egui::ScrollArea::vertical().max_height(room).show(ui, |ui| {
                layer_list(ui, layers, visibility, &mut self.editor_layer, &mut self.editor_group, actions);
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
    }

    /// the cheats window over a game, when open, its cheats by name, whether
    /// each is on, and their notes.
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
                    // what it does to the game and who made it, in sight
                    // before it is turned on
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

    /// the multiplayer window, when open: the room the player is in, or
    /// joining one and opening one.
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

    /// the settings window, when open.
    pub fn settings(&mut self, ctx: &egui::Context, settings: &mut Settings) -> Vec<Action> {
        let mut actions = Vec::new();
        let before = settings.clone();
        let mut open = self.settings_open;
        // taller than a screen has room for, so it scrolls, and it can be
        // dragged to any size that fits
        let room = ctx.content_rect().height() - 16.0;
        egui::Window::new("Settings")
            .id(egui::Id::new("settings"))
            .open(&mut open)
            .vscroll(true)
            .default_size([460.0, room])
            .max_height(room)
            .show(ctx, |ui| {
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
                ui.checkbox(&mut settings.hardware_rasterizer, "Draw the 3D on the GPU");
                ui.add_enabled(
                    settings.hardware_rasterizer,
                    egui::Slider::new(&mut settings.resolution, 1..=8).text("Resolution").suffix("x"),
                )
                .on_hover_text("How many times the console's resolution the 3D is drawn at. Past what the window shows, the extra pixels smooth the edges, at a cost to the GPU that grows fast.");
                ui.add_enabled(settings.hardware_rasterizer, egui::Checkbox::new(&mut settings.texture_packs, "Texture packs"))
                    .on_hover_text("Draw a game's texture pack, made for Citra or Azahar, put in the textures folder of the game's mods folder.");
                ui.add(egui::Slider::new(&mut settings.scale, 1..=6).text("Window scale"));
                egui::ComboBox::from_label("Screens").selected_text(settings.layout.name()).show_ui(ui, |ui| {
                    for layout in Screens::ALL {
                        ui.selectable_value(&mut settings.layout, layout, layout.name());
                    }
                });
                egui::ComboBox::from_label("Filter").selected_text(settings.filter.name()).show_ui(ui, |ui| {
                    for filter in Filter::ALL {
                        ui.selectable_value(&mut settings.filter, filter, filter.name());
                    }
                })
                .response
                .on_hover_text("How the screens' pixels look scaled up: smooth, sharp with soft edges, or plain squares.");
                ui.checkbox(&mut settings.integer_scale, "Scale by whole numbers only")
                    .on_hover_text("Every pixel gets the same size, with a border around the screens.");
                egui::ComboBox::from_label("Top screen shape").selected_text(settings.top_shape.name()).show_ui(ui, |ui| {
                    for shape in TopShape::ALL {
                        ui.selectable_value(&mut settings.top_shape, shape, shape.name());
                    }
                })
                .response
                .on_hover_text("Shows the top screen wider than the console's. Games still draw it at the console's shape, so only turn this on with a widescreen code for the game, which draws a wider scene to match. Kid Icarus: Uprising draws a wider view by itself. The window's shape follows the window, with the top screen alone, up to 32:9.");
                egui::ComboBox::from_label("3D").selected_text(settings.stereo.name()).show_ui(ui, |ui| {
                    for stereo in Stereo3d::ALL {
                        ui.selectable_value(&mut settings.stereo, stereo, stereo.name());
                    }
                })
                .response
                .on_hover_text("The top screen in 3D, for games that draw it: red and cyan for glasses of those colors, side by side for 3D TVs and headsets, alternate rows for passive 3D monitors. Off, games draw flat, as with the console's 3D slider down.");
                let stereo = settings.stereo != Stereo3d::Off;
                ui.add_enabled(stereo, egui::Slider::new(&mut settings.depth_3d, 0..=100).text("Depth").suffix("%"))
                    .on_hover_text("The console's 3D slider, how deep games draw the 3D.");
                ui.add_enabled(stereo, egui::Checkbox::new(&mut settings.swap_eyes, "Swap eyes"))
                    .on_hover_text("Each eye gets the other's picture, for viewing cross-eyed, glasses worn the other way round, or a monitor whose rows start with the other eye.");
                ui.label(
                    RichText::new("F9 switches them while playing, F10 between the top and the bottom screen alone.").weak(),
                );
                ui.label(RichText::new("The presenter changes the next time Zakuro starts, the 3D with the next game.").weak());

                ui.separator();
                ui.heading("Emulation");
                egui::ComboBox::from_label("Language").selected_text(settings.language.name()).show_ui(ui, |ui| {
                    for language in Language::ALL {
                        ui.selectable_value(&mut settings.language, language, language.name());
                    }
                })
                .response
                .on_hover_text("The console's system language, which titles pick their localised text and files by. It changes with the next game started, or Reset.");
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
                ui.radio_value(&mut settings.recompiled, true, "Recompiled code when there is some");
                ui.radio_value(&mut settings.recompiled, false, "Interpreter only");
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

    /// a message box, when there is something to tell.
    /// the keyboard a game opened to have text typed in, it waits until a
    /// button closes it.
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

    /// asks whether to download a compiler, there being none to recompile
    /// with.
    pub fn compiler_offer(&mut self, ctx: &egui::Context) -> Option<Action> {
        let index = self.compiler_offer?;
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
                        action = Some(Action::DownloadCompiler(index));
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
        egui::Window::new("Zakuro")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(text.as_str());
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

/// a recompile's progress bar with where it is.
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

/// a key as a person would call it.
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

    /// the settings stay inside a short screen, scrolling in it instead of
    /// running past its bottom.
    #[test]
    fn the_settings_fit_a_short_screen() {
        let ctx = egui::Context::default();
        let mut menus = Menus { settings_open: true, ..Menus::default() };
        let mut settings = Settings::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(800.0, 360.0));
        for _ in 0..3 {
            let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
            ctx.run_ui(input, |ui| {
                menus.settings(ui.ctx(), &mut settings);
            })
            .drop_without_applying_deltas();
        }
        let window = ctx.memory(|memory| memory.area_rect(egui::Id::new("settings"))).unwrap();
        assert!(screen.contains_rect(window), "{window:?} in {screen:?}");
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
        rules: Option<PauseRules>,
        cauldron: Option<CauldronRules>,
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
                cauldron: None,
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
            let cauldron = self.cauldron;
            let controls = self.controls.clone();
            let mut drawn = NavigationEditorResponse::default();
            self.ctx
                .run_ui(input, |ui| {
                    drawn = menus.layout_editor(ui.ctx(), rules, cauldron, layers.clone(), visibility.clone(), controls.clone(), [true; 4], 1.0, Some(canvas), true, &mut actions);
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
        editor.cauldron = Some(mouse.cauldron_rules());
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
        // the bottom right corner resizes the layer instead of moving it
        let handle = editor.point(75.0, 25.0);
        editor.press(handle);
        let corner = editor.point(85.0, 35.0);
        let actions = editor.frame(vec![egui::Event::PointerMoved(corner)]);
        println!("resizing from {handle:?} to {corner:?} gave {actions:?}");
        assert_eq!(editor_move(&actions), Some((0, [30.0, 10.0, 60.0, 30.0])), "{actions:?}");
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
        // the group spans 30..170 by 10..30; its right member's corner grows it
        let handle = editor.point(168.0, 28.0);
        editor.press(handle);
        let actions = editor.frame(vec![egui::Event::PointerMoved(editor.point(182.0, 48.0))]);
        let scaled = moves(&actions);
        assert_eq!(scaled.len(), 2, "{actions:?}");
        for ((index, got), want) in scaled.iter().zip([[30.0, 10.0, 55.0, 40.0], [129.0, 10.0, 55.0, 40.0]]) {
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
                menus.game(ui, "Game", None, false, false, &[], None, None, None, layers.clone(), vec![true], Vec::new(), [true; 4], 1.0, Some([500.0, 0.0, 400.0, 240.0]), true, true);
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
        for rules in [None, Some(PauseRules::default())] {
            let mut editor = Editor::on(layers.clone(), [0.0, 0.0, 400.0, 240.0], screen);
            editor.rules = rules;
            for _ in 0..3 {
                editor.frame(Vec::new());
            }
            let (title, id) = if rules.is_some() { ("Menu editor — Pause", "menu_editor_pause") } else { ("Menu editor", "menu_editor") };
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
/// adds it to the selection or takes it out. returns the checkboxes.
fn layer_list(
    ui: &mut egui::Ui,
    layers: &[(String, [f32; 4])],
    visibility: &[bool],
    selected: &mut Option<usize>,
    group: &mut Vec<usize>,
    actions: &mut Vec<Action>,
) -> Vec<egui::Response> {
    let mut boxes = Vec::new();
    if layers.is_empty() { return boxes; }
    ui.separator();
    ui.label(RichText::new("Layers").strong());
    ui.label(RichText::new("Ctrl or Shift selects several to move or scale together.").small().weak());
    let adding = ui.input(|input| input.modifiers.command || input.modifiers.shift);
    for (index, (name, _)) in layers.iter().enumerate() {
        ui.horizontal(|ui| {
            let mut visible = visibility.get(index).copied().unwrap_or(true);
            let response = ui.checkbox(&mut visible, "").on_hover_text("Draw and use this layer in the game.");
            if response.changed() {
                // the toggled layer is the one the editor is about now, so
                // it can be shown again or checked on the canvas deliberately.
                *selected = Some(index);
                *group = vec![index];
                actions.push(Action::EditorLayerVisible(index, visible));
            }
            boxes.push(response);
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
        });
    }
    boxes
}

/// the boxes a drag asks for, by `by` layer units from where each began:
/// moved together, or scaled about the group's top left corner, so a
/// group keeps its arrangement as it grows or shrinks.
fn dragged(start: &[(usize, [f32; 4])], by: [f32; 2], resizing: bool) -> Vec<(usize, [f32; 4])> {
    if !resizing {
        return start.iter().map(|&(index, to)| (index, [to[0] + by[0], to[1] + by[1], to[2], to[3]])).collect();
    }
    let Some(&(_, first)) = start.first() else { return Vec::new() };
    if let [(index, to)] = start {
        return vec![(*index, [to[0], to[1], (to[2] + by[0]).max(1.0), (to[3] + by[1]).max(1.0)])];
    }
    let bounds = start.iter().fold(first, |b, &(_, to)| {
        let (x, y) = (b[0].min(to[0]), b[1].min(to[1]));
        [x, y, (b[0] + b[2]).max(to[0] + to[2]) - x, (b[1] + b[3]).max(to[1] + to[3]) - y]
    });
    let scale = [(bounds[2] + by[0]).max(1.0) / bounds[2].max(1.0), (bounds[3] + by[1]).max(1.0) / bounds[3].max(1.0)];
    start.iter().map(|&(index, to)| (index, [
        bounds[0] + (to[0] - bounds[0]) * scale[0],
        bounds[1] + (to[1] - bounds[1]) * scale[1],
        (to[2] * scale[0]).max(1.0),
        (to[3] * scale[1]).max(1.0),
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
