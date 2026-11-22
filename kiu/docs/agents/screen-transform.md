# Screen Transform

The contract every game-owned Screen follows to appear on the top screen. Vocabulary is in `GLOSSARY.md`; the decision and its alternatives are in `kiu/docs/adr/0005-screen-transform-pipeline.md`.

## Scope

Applies to every **In-Game Menu**, **Scripted Menu**, and the Fiend's Cauldron. The Frontend Menu is out of scope: it is emulator UI, not game UI.

## Stages

One orchestrator runs these stages in order. Capture happens in the bottom pass, replay in the top pass, input in the game's input path; the orchestrator owns the ordering and all state between them.

| Stage | Input | Output |
| --- | --- | --- |
| detect | frame, game state | one active `ScreenSpec` |
| capture | bottom-pass calls or scene models | recorded draw units for the Screen |
| seat | registered width and aspect | one `ScreenLayout` |
| replay | `ScreenLayout` | top-pass drawing |
| map input | pointer, keys, buttons, `ScreenLayout` | native touch or menu calls |

No stage may branch on a Screen id or registry index. Behaviour that differs between Screens is declared on the row.

## ScreenSpec

```rust
struct ScreenSpec {
    id: Screen,                 // canonical name from GLOSSARY.md
    family: Family,             // InGame | Scripted | Cauldron
    identity: Identity,         // CallSites | Vtable | TaskProbe
    first_call: usize,          // offset into a shared call table
    input: InputMode,           // PressA(addr) | TouchesOnly | ClickAsTouch
    outline: Option<OutlineFn>, // fn(&mut System, u32, Found) -> Vec<MenuCall>
    activate: Activate,         // game call or native touch
    layout: LayoutKind,         // row-driven: None, FileSelect, Chapter, Generic, Group, Idol
    wheel: Option<WheelFn>,
    fallback: &'static str,     // the env flag that restores native drawing
}
```

The registry now carries all 28 Scripted Menus plus Pause, Retreat Dialog, the Fiend's Cauldron and the Midstage Loading: `id`, `family`, `identity`, `first_call`, `input`, `fallback`, `outline`, `activate` and `layout`. Pause and Retreat Dialog share one call table, so Retreat Dialog starts at `first_call = 1`. Scripted Menu layout data is row-driven (`LayoutKind`); `menu_top` still reads it through its index entry point, and the editor-facing anchor fields and `wheel` are pending.

`Identity` selects one of three detector adapters; all three are implemented:

- `CallSites(&'static [ToTop])` — pause and dialogs.
- `Vtable { vtable, update }` — Scripted Menus.
- `TaskProbe { show }` — Fiend's Cauldron.

`Screen` is a typed enum of glossary names, so a request, a test, and the editor refer to the same value.

## Placement

`ScreenLayout` is an ordered list of placed draw units:

```rust
struct Placement {
    from: [f32; 4], // source x, y, width, height in the game's screen pixels
    to: [f32; 4],   // target x, y, width, height in top-screen pixels
}

struct PlacedUnit {
    unit: DrawUnit,   // Model | CallSite | Layer | QuadGroup
    place: Placement,
    shown: bool,
}

struct ScreenLayout {
    units: Vec<PlacedUnit>,
}
```

Seat produces the list once per frame. Replay draws from it; input hit-tests the same list. A second geometry calculation for input is forbidden: that is how hover drift returns.

Pause uses five editable `PlacementRule`s keyed by `QuadRole` (span, centre, left button, right button, corner). A rule carries visibility, the source box, the target box at 400x240, an anchor, and a scale mode (`Fixed`, `Narrow`, `Stretch`); `PauseRules::layout` resolves them for the current width. Hidden rules yield no Placement for drawing or hit-testing. The editor mutates these rules, not the resolved rectangles.

The Retreat Dialog has its own two rules, `DialogRules` (span: the dimming; panel: the box, its No and Yes, and the question). `ScreenTransform::screen_layout` picks Pause's or the dialog's by the call table entry being drawn, and touch reads the dialog's while it is the active Screen, so Pause's rules never move the dialog and the dialog's never move Pause. The panel places both x and y (Pause places x only); the words, drawn through the 3D renderer, follow it through their projection (`place_projection`), and a hidden panel draws none of them. F2 on the dialog opens **Menu editor — Retreat Dialog** with Save layout, Reset and Export; `menu-layout.toml` keeps its rules under `dialog`, and older files load the defaults. `kiutrace` edits them with `dialogrule` and `dialogvisible`.

The Midstage Loading, between an air and a land battle, has four rules keyed by `LoadingRole` in `LoadingRules`: Tip bar (0), Tip (1), Loading orb (2) and Midstage Progress (3). It is the dialogs' 2D context with its loading bit set, so its top pass marks it shown and every part reads its rule only on that pass: the Tip bar's and Loading orb's flushes are moved quad by quad, the Tip goes into its camera's projection (`place_projection` at the top's 400), and Midstage Progress, the bottom screen's words, comes through the Retreat Dialog's text calls but takes its own rule instead of the dialog's panel. The bottom pass draws those words for three frames after the top's last loading pass; they keep the loading's rule through that grace. A hidden rule draws its part nowhere. It has no Controls and takes no touches. By default the bar and the tip stay where the game draws them, Midstage Progress is above them at four fifths of its size, and the orb sits by the right edge above the bar, so no part covers another at 5:3, 16:9 or 21:9. F2 on the loading opens **Menu editor — Midstage Loading** with the four parts on the canvas, Visible checkboxes, Save layout (`loading` in `menu-layout.toml`, absent in older files), Reset and Export. It needs Pause on the top screen (`ZAKURO_PAUSE_TOP`), which also carries its words.

The Fiend's Cauldron uses nine editable `PlacementRule`s keyed by `CauldronRole` (top picture, panel, bar, dial, Back, Move Out, rest, ring, hint) in `CauldronRules`. The ring (`MenBetHeartBt0010`, the bottom screen's background) takes no touches and is drawn before the top's 3D, so the board, the pot and every other part cover it. The hint is the top's own line of text ("Bet hearts to…"), drawn through the 3D renderer: its Hint placement goes into the projection of that line's camera call (`cauldron_hint`, `place_projection` at the top's 400 across), and a hidden Hint draws no line; its default box is the line's own, at the size the game draws it. A saved list of Cauldron rules from before a role was added loads with that role's default. They use the `Fit` scale mode: the target box is authored for a 427-pixel-wide shown screen (16:9), shrinks evenly below it, and `anchor`/`anchor_y` pick the left/right and top/bottom edge it keeps; the result is in the same 400×240 top-screen space as Pause. The panel's and dial's sources are where they sit at intensity 0 and slide with the pot's value; `CauldronRules::layout` centres both targets across on the bar's point for that value, wherever the bar is placed, so only their height, size and vertical place come from their rules. They have no Placement without a value. The pot's task probe seats this layout once per frame; drawing, hit-testing and Cursor lighting all read it. What the pointer does over each part — the native Cursor name, the touchable sub-box (Move Out's button), and the dial's drag along the bar — is the `CAULDRON_CONTROLS` table, in hit-test order; it is not editable rule data. The frontend sends every top-screen touch through `AimMouse::top_screen_touch`, which picks Pause, the Cauldron or a Scripted Menu. `kiutrace` edits the rules with `cauldronrule` and `cauldronvisible`.

Cauldron parts are moved and scaled, not cropped: each part is drawn through the whole top screen, with its camera mapping its rule's source box onto its target box, so the box says where the part goes and what the pointer hits, and nothing of the part is cut at its edges. A part is isolated by what it draws, not by a window: its model group, and for the bar and the dial, which share `MenBetHeartBt0002`, mesh nodes (the dial is `ber`, its stem and dial, `cursor` and `noheart`; the bar is the rest, with the "Try This?" and crown models). The board is the one exception: it is one of a strip of boards, one per intensity, which the hidden frame masks, so its source box is kept as that mask. The **Not enough hearts** tag (the `noheart` node and the bottom pass's one line of text) belongs to the dial; every other replay skips the text. The text's anchor is projected through each replay's camera, so it is taken from the native bottom pass instead and moved and scaled by the dial's placement.

On the Cauldron, F2 opens **Menu editor — Fiend's Cauldron**, listing its nine parts instead of stale Scripted Menu layers. Drag a part to move it; its corner handle resizes it, keeping its aspect ratio; the arrow keys nudge the selection by 1 pixel (10 with Shift); Visible hides drawing and touch together. Panel and Dial stay centred on the bar's point for the intensity, so dragging them moves them only up and down, and moving the bar takes them with it. Canvas edits are converted back to design-width rules and reseat drawing and input immediately. **Save layout** stores Cauldron rules alongside Pause rules in `menu-layout.toml`; older files default to the committed rules. Console state loads and game resets retain edits. Normal launches use the committed layout (see Committed layout below). Reset restores defaults, and **Export Rust + change request** writes `menu-layout-export.txt`. Pause and Retreat Dialog expose no stale Scripted Menu layers.

Anchors are layout-rule data, not runtime fields: edge-anchored Controls and centred content compute their target rects for the current width, and the resulting `Placement` is the concrete mapping. The editor edits those rules, so a layout keeps its shape at 5:3, 16:9, 21:9 and dragged widths.

### One view

The pipeline draws one view: the top screen as shown. Kid Icarus draws every top-screen pass twice, into the left eye (`0x18080000`, shown) and the right eye (`0x18000000`, never shown with the 3D off), even with the slider down. The GPU drops every draw into the right eye's target at the top screen's size (`aim_mouse::RIGHT_EYE`, set each frame while the mouse module is attached); the bottom screen reuses that VRAM at 240×320 and is kept. Replays are made once, into the shown eye (the Retreat Dialog's words at `0x007258C0`). The 3D settings are hidden and always off.

## Behaviour invariants

Every transformed Screen satisfies all eight. Exceptions are declared on the row and named in the acceptance scenario.

1. Pointer over a Control lights its Outline.
2. Pointer off every Control clears the light.
3. A click activates exactly what native A activates, or a native touch where the Screen is touch-only.
4. Press-and-hold or drag remains a native touch.
5. Keys and gamepad restore the native Cursor.
6. Wheel scrolls a List when the Screen has one.
7. Focus loss or Screen change fires a Pointer Reset.
8. Disabling the Screen's `ZAKURO_*` flag restores untouched native dual-screen drawing.

## Migration

Order: Pause, Retreat Dialog, Options family, Scripted Menus (simple button Screens first, Armory lists and boards last), Fiend's Cauldron.

For each Screen:

1. Add its `ScreenSpec` row and detector adapter.
2. Route capture, seat, replay and input through the orchestrator.
3. Keep the existing flag as rollback.
4. Prove parity (below) and delete the legacy path.

A Screen is migrated only when all five stages run through the pipeline. Partial-stage wiring counts as in progress, not done.

## Verification

- Registry test in `zakuro-core`: unique ids, every field present, every migrated Screen has a row, no routing gaps.
- One `kiutrace` scenario per Screen: save state, hover point, click, expected Outline and activation, Pointer Reset, and the fallback flag.
- Screenshots before and after a cutover at 5:3, 16:9, 21:9 and a narrow window.
- No pop-in on entry: every adapted Screen has a model-name prefix in the entrance-detection list in `scene()` (`aim_mouse/menu_top.rs`), so the new layout draws from the first frame its graphics load, before the Screen's input update runs. Check by replaying the transition into the Screen with a `shot` every few frames: the original layout must never appear. On a new scene's first frame the bottom camera has no projection until the game's own bottom pass draws through it, so bottom parts wait one frame (`draw_part`); drawn earlier they would show unplaced.

Existing save states are the baseline; a Screen without a scenario is unported.

## Committed layout

`kiu/menu-layout.toml` is built into the executable and is the layout every normal launch uses: layer moves, hidden layers, navigation links and the Pause, Retreat Dialog, Cauldron and Midstage Loading rules. `--menu-editor` reads the data folder's `menu-layout.toml` instead, falling back to the committed one until **Save layout** writes it. To ship an edit, Save layout, copy the data folder's `menu-layout.toml` over `kiu/menu-layout.toml`, and rebuild.

## Editor

Dev-only egui overlay, opened with `--menu-editor` and toggled with F2 anywhere, or by the Layout editor button in the Esc menu. It draws a canvas over the live top screen, outlines the active Scripted Menu's layers, and lists them by model name. Dragging a layer moves it; the bottom-right handle resizes it. The edit is kept per screen and part index and applied when the layout is rebuilt, so drawing and hit-testing follow it. Pause rules remain the numeric `PlacementRule` editor.

The editor opens independently of the Esc overlay. A drag selects the topmost layer under the press, or one already selected (such as from the list pane) even when another covers it, so covered items can still be dragged. Its lower-right handle resizes from the original placement, keeping the selection's aspect ratio, and releasing ends the edit. The arrow keys nudge the selected item(s) by 1 pixel, or 10 with Shift, while no text field is focused. Changing the layer list clears the previous selection. Editor windows sit above the canvas so their Controls take input, and long layer/rule lists scroll within the available window height. Both editor windows can be closed with their X; saving is available for Scripted Menu placements and for Pause rules.

Ctrl or Shift adds a layer to the selection, on the canvas or in the list, and takes a selected one out again. Dragging any selected layer moves them all; a selected layer's handle scales the whole selection about its top-left corner, keeping the layers' arrangement. A plain click or a drag on an unselected layer starts over with that one. Navigation edits the last layer picked.

Every Screen except File Select and Chapter Select also lists the game's own top-screen drawing as its last layer: **Details card** where the menu shows weapon or page details beside its Controls, otherwise **Top screen** (the scene behind a hub). Moving, scaling or hiding it changes where that native pass is placed and clipped; it takes no pointer input and has no navigation. Being last, it left every earlier layer's saved index unchanged.

Every Scripted Menu item has a **Visible** checkbox, including HOME hint, backgrounds, information Boards and Hint strips. Pause exposes the same checkbox inside each rule group. Hidden items remain in the list with their geometry and stable indices so they can be shown again; their gray editor bounds have no drag handle and cannot intercept canvas selection or dragging. Hidden Scripted Controls cannot receive pointer input or be chosen as explicit navigation destinations. Visibility changes cancel any held Synthetic Touch. Hiding is a visual layout setting, not deletion of native game state or keyboard/controller bindings.

**Folders** group a Scripted Menu's layers in the list: **New folder** puts the selection in one, a folder's name selects its layers (double-click renames it), its checkbox shows or hides them all, **Add** moves the selection in and **Take out** or **Ungroup** removes layers or the folder. A **Linked** folder keeps its layers selected together, so picking any of them on the canvas or in the list moves, scales and nudges them all. Folders are kept per Screen in `menu-layout.toml` under `folders`; the Cauldron, Pause and the Retreat Dialog have none. **Undo** and **Redo** (Ctrl+Z, Ctrl+Shift+Z or Ctrl+Y, and buttons over the list) step through drags, nudges, visibility changes and folder edits on the current layer list; changing Screens clears them.

**Save layout** persists hidden Screen/part pairs even when no geometry was moved, plus each Pause rule's visibility. Loading a console state keeps visibility; the next editor-enabled launch restores saved visibility. Older files without visibility data default to all items visible. Export includes visibility alongside geometry.

Animated model Controls use those same live placements for collision solving and drag bounds, including editor overrides; they do not rebuild default Solo Hub geometry for physics. Main Menu and other matching model Controls share that path. Held host drags retain their unbounded mapped coordinates past the native bottom screen's 320×240 touch limits. The native bottom-screen copy remains a native viewport; its clipping does not limit movement or replay on the top screen. Editing a placement invalidates cached pointer candidates so Hover is remapped on the next input update.

The `kiutrace` command `layer <index> <x> <y> <width> <height>` exercises the same editor mutation as the overlay; `layers` prints indices and placements. Use it with `mhover`, `expectmenu`, `click`, and `mdragtop` to check drawing and interaction from a save state.

`kiutrace` also supports `visible <index> <0|1>` for Scripted items and `pausevisible <span|center|left|right|corner> <0|1>` for Pause groups. Hiding every File Select item produces an empty top screen rather than falling back to the original unfiltered parts.

File Select exposes sixteen independent Placements. Existing indices remain Book I–III (0–2), Back (3), How to Play (4), HOME hint (5), Player information Board (6), and Bottom-screen background (7). Added layers are Paper background (8), Coloured ribbons (9), Top-left decoration (10), Bottom-left decoration (11), Bottom-right decoration (12), Right-edge decoration (13), Feather decoration (14), and Coloured crest (15). The Board moves its black artwork, file numeral and player information without moving the backdrop or edge decorations. These overlapping parts are isolated by mesh ownership, not screen crops; only the Board replays player-information glyphs. Added layers start at the default Board placement; older saved Board edits do not reposition them. Each layer supports the existing move, resize, visibility and Save layout controls. The books retain native 3D depth/material ordering; their meshes and player-name text follow the same Placement without clipping turning covers to the editor box. Decorative layers take no activation or navigation input. Leaving every Control clears the host's previous Hover result.

Book mesh replay keeps the full shown viewport and puts the Placement in the projection, so Book I's turning spine can cross the native 320×240 Board edge. Expanding only the scissor leaves that spine cut. The replay permits one book-width/height beyond each native Board edge, excluding the model's separate off-screen open-book view rather than clipping at the Control box. Font anchors already include the placed projection; ownership checks map them back to native coordinates before choosing a book's name. Drawing retains the projected coordinates, and HOME text stays out of book replays.

File Select's native black cross-screen book connectors are omitted from the transformed drawing. They are decorative meshes, not the books or their native selection/activation state, and cannot line up once the Boards and books are edited independently.

File Select's former single `controls #0` is replaced by these independent entries. Re-edit saved File Select placements made with that older grouping; other Screen entries retain their indices. Save layout and console-state reload retain the new per-Control placements through the existing layout format.

Armory — Gear exposes six additional top-screen Placements after its existing entries: Paper background (5), Silhouette (6), Crest (7), Ornaments (8), Page title (9), and Weapon (10). Each supports move, resize, visibility and Save layout independently. Node ownership is declared on the Screen's layout row; the native Details card draws unowned nodes, including the Powers grid. Split layers replay meshes without duplicating Hint glyphs, and keep the native viewport mapping so their artwork matches their Placement. Existing saved indices stay unchanged; new layers start at the default Details card placement.

Armory — Powers exposes Paper background (7), Power set board (8), Power preview (9), Power shape (10), and Description background (11). These artwork layers support the same independent move, resize, visibility and Save layout controls. Details card (6) owns the text and remaining decoration; Controls (0–5) keep their existing Placements. A layer retains the game's native visibility, so artwork used only in another power/set mode can be empty in the current mode. Mesh-only artwork replays skip the entire top text call: suppressing only individual glyphs still repeats text setup and can exhaust Powers' drawing buffer. Acceptance: on a full Powers board, move the Paper background and Power preview independently, hide/show the background, then use Back to return to Gear.

Idol Toss is one Screen, from the Solo Hub and from the Vault alike: its class runs both registry rows' update thunks (22's, then 25's) every frame for either, and only 22's while leaving, so the rows cannot tell the two apart. Every frame, its leaving and its play-coin Dialog draw and save as Screen 25 (`screen_of_row`), and its entrance draws Screen 25 from the first frame its models load. It exposes Counters (0), Back (1), Launcher (2), hint (3) and Top screen (4), then Fountain (5, the water and the gold pedestal) and Egg queue (6, the queued eggs on their rail). Launcher, Fountain and Egg queue share one transform by default (`idol_toss`), at the bowl's scale, so the fountain stands under the bowl and an egg dragged out of the queue keeps its size and place as it passes from the Egg queue to the Launcher; the queue runs down the right edge and its rail ends at the bottom edge, as it did at the bottom screen's. Fountain and Egg queue draw before the other groups; only Fountain takes no touches. A held egg drag starts through the Egg queue's Placement and transfers to the Launcher's when it reaches the bowl, without releasing the native touch. Back, Launcher, Fountain and Egg queue replay through the whole shown screen with placed projections, so their animated meshes are not cropped at their hit boxes. The coin dialog adds Dialog before the hint, moving the later indices up by one.

During Idol Toss's native launch/reveal phases, Launcher and Fountain are unavailable: replay and hit-testing both omit them. Egg queue and Back retain their existing Placements. Returning to egg tossing makes Launcher and Fountain available again, subject to their saved editor visibility; the transition never overwrites those settings.

Idols (the Vault's, Screen 26) is one Board for both of its pages, the List and one idol's description. Its bottom Placements are Background (0), Controls hint (1), Title bar (2), Zoom bar (3), Description (4, the white panel and its words), Page counter (5), List frame (6), Rows (7, with the Cursor and Row mask), Scroll bar (8), Back (9) and AR (10); the top's are Details card (11), Backdrop (12), Shadow (13), Idol (14) and Emblem (15). The Emblem, the category's mark on the backdrop, is a texture on the backdrop's only mesh, so it is a texture layer (`LayoutKind::Board`'s `texture_layers`), not a node layer: it draws nothing itself and places the texture through the backdrop material's texture Scale and Translate. It shows only inside its own quad (native 248, 25, 114×113), so the editor clamps its box there, and it follows the Backdrop when that moves. Acceptance: from `trace/idols-viewer.state`, set Backdrop to 0 0 400 240 and Emblem to 248 81 57 57; the emblem draws at half size in the quad's lower left with no streaks, and hiding Emblem removes it. The floor shadow is the game's `CircleShadow` sprite, not a model, and draws in every scene replay; the Shadow layer owns it (`CIRCLE_SHADOW`), and every other replay skips its draw, so the Backdrop can be stretched over the whole top screen without carrying a copy. Acceptance: from `trace/idols-viewer.state`, set Backdrop to 0 0 400 240; one shadow stays under the idol and hiding Shadow removes it. Layers 0–5 are bottom node layers: `LayoutKind::Board`'s `bottom_layers` owns `MenColViewerBt0000`'s nodes, named in `group` by layer name and replayed with only that model. A node layer replays the bottom text pass only where it owns words — Title bar, Description and Page counter on the description page; the List frame and Rows on the List — and where boxes overlap the smallest box owns a glyph. Replaying text in every layer overflowed the game's drawing buffer and blacked out the frame. A page's absent models draw nothing. The Screen keeps no Outline: a click is a touch at the Placement-mapped point, so a Row or a moved Back works where it is shown. Acceptance: on the List, move Rows and Back apart, click the second Row (Stealth Claws, 2/9), then click Back at its new place to return to the Vault.

A left press on the Idol layer (14), off every Control, Orbits the Idol View instead of touching: the drag writes the game's own angle and tilt at 0.3° a window pixel (PC Preferences' Idol View orbit sensitivity), inverted as the Land Battle camera is, and stops the idle spin. The drag is captured until release; released while moving it coasts to rest over about 1.5 s. Acceptance: from `trace/idols-viewer.state`, `mdragtop 150 40 350 40 20` adds 60° to the angle and leaves the spin off; 11 `mpress` steps of 10 px then `mhover` keep turning for over 30 frames and then stop.

Power Portrait (the Vault's, Screen 27) is one Board. Its bottom Placements are Background (0, `MenCmnBg0000`), Message (1, the line of words), Completion badge (2, banner and ring), Completion number (3) and Back (4); the top's are Details card (5), Frame (6), Checkerboard (7) and Portrait pieces (8). The Message layer owns `pPlane1`, a node that draws nothing, so the bottom text pass replays only there. Node layers are cut at their boxes, so each default box covers its artwork. The Screen keeps no Outline: a click is a touch at the Placement-mapped point. Acceptance: from `trace/powerportrait.state`, move Frame, Checkerboard and Portrait pieces apart and Message and Completion number away from the badge; each moves alone, hiding Background keeps the words, and a click on Back at a moved place returns to the Vault.

Armory — Fuse Weapons (Screen 17) is a Board split on both screens. Bottom, back to front: Weapon A background (0), Weapon B background (1), Frame mask (2), Frame (3), Weapon A scroll bar (4), Weapon B scroll bar (5), Back (6), Fusion flash (7), Weapon A Cursor (8), Weapon B Cursor (9), Grid Cursor (10), Sort order (11), Fusion grid (12), Weapon A list (13), Weapon B list (14), Fusion dialog (15) and Window (16); then hint (17) and Details card (18). Top: Paper background (19), Ornament (20), Weapon A (21), Weapon B (22), Filled bonus slots (23), Weapon A bonus slots (24), Weapon B bonus slots (25), Result bar (26), Result rings (27), Result rays (28), Result shadow (29), Result weapon (30) and Sparkles (31). Weapon A's list is the row along the top, Weapon B's the column at the left. Bottom layers are cut at their native boxes; a Cursor's box covers everywhere it can go. The game's phase word (`FUSE_PHASE`) decides who takes the pointer: the Board's Controls (Back, both lists and scroll bars, the grid, Sort order), or only the Fusion dialog while it asks Fuse/Cancel or Equip? or fuses. Fusion dialog, Fusion flash and Window draw only then, which keeps the Board's frames to 14 bottom replays (17 froze the loading after an equip). Sort order and the Fusion dialog carry the bottom words. The Result bar is the bar the hint's words sit on, so by default it takes the hint's Placement (`HINT_BACKING`). Acceptance: from the Screen, move Weapon B list and the Fusion dialog away; a click on a weapon in the moved list opens Fuse/Cancel, Fuse and then Yes clicked in the moved dialog fuse and equip, and the Board returns after the loading. Move Weapon A, Result weapon and Result bar apart and hide the Paper background; each moves alone and the words stay.

Armory — Weapons exposes Paper background (7), Description board (8), Category emblem (9), Stats card (10), Weapon shadow (11), Weapon (12) and Equip flash (13). Details card (6) owns the words (name, stats, description). Every top model is owned by a layer, since anything left to the Details card draws over them. The Details card replays last whenever a Screen has split layers, and through the guard-band projection, so its words keep their native width. Acceptance: on `trace/armory-weaponlist.state`, move the Stats card, Description board, Category emblem and Weapon apart, then hide the Paper background; each moves alone and the words stay. To split another Screen, follow `.claude/skills/split-top-screen/SKILL.md`.

Armory text Controls emit only glyphs anchored within their native source rectangle. Scissoring alone still allocates unrelated glyphs on every replay and can exhaust the game's shared drawing buffer, freezing Powers during entry with larger inventories. Drawing and input keep the same Placements.

### Explicit directional links

Select an addressable model Control and use **Navigation** to set its Up, Down, Left and Right destinations. **Native default** removes just that direction's override; **Stay on this Control** is a self-link. Geometry edits do not automatically change explicit links.

Individually editable Controls and native links cover File Select, Main Menu, Solo Hub, Arms Altar, Options, Practice Range, Vault, Other, Controls/More Controls, Reticle/Camera, Subtitles, Volume, Profile and Profile Yes/No. Child Controls keep their native Board arrangement by default. The Controls of Options, Other, Controls/More Controls, Reticle/Camera and Subtitles are drawn through the full shown viewport with a placed projection (`drawn_cut` in `menu_top.rs`), so frames, animated Outlines and Try It! are not cut at their input rectangles; their boxes still set layout and hit-testing, and they skip the bottom text pass. The Armory Boards' Controls are drawn the same way, such as the weapon category tiles and Gear's Powers, except those whose words come from the bottom text pass (`BOARD_TEXT`, such as Gear's Weapons card and the weapon list): that text follows the box, not a whole draw's projection, so they stay cut (`part_cut`). Volume (labels and slider pieces share one mesh), Profile, Profile Yes/No and the SpotPass Dialog (bottom-pass words) are still cut at their boxes (`board_cut`). Controls and More Controls share stable saved indices; rows on the other page are marked **inactive**, excluded from drawing, hit-testing and navigation destinations.

The SpotPass Dialog exposes its caption, No and Yes separately. No and Yes have independent Placements and Left/Right links. Its native choice box has no Up/Down actions; the editor marks those directions unavailable rather than storing inert links. Other unbound and grouped Screens retain native navigation.

Links redirect native graph lookups, including the per-Screen inline lookups, rather than rewriting pad input. Native Outline updates and A activation remain the game's. Volume uses its native queued-name path when a link overrides a slider direction; without a link, Left/Right still adjusts the slider, while a self-link leaves its value unchanged. Diagonal directions retain their native routes. Close the editor with F2 to test game input. **Save layout** stores links alongside placements and Pause rules; older layout files default to no links. Links survive console state loads and game resets; normal launches take them from the committed layout.

File Select keeps native repeat and A activation, but redirects a linked cardinal action through its native Outline helper instead of only substituting a graph slot: that helper also changes the selected book index and animates the correct cover. A jump from Book I to Book III therefore changes both the Cursor and the book, while a self-link changes neither.

`kiutrace` supports `navlink <from> <up|down|left|right> <to|native>` using the indices printed by `layers`. Use `hold`, `circle`, and `expectword <address> <value>` to check the native Cursor, rather than the pointer-only `expectmenu`. The Main Menu acceptance replay redirects Solo's Right to Vault, checks all four cardinal directions and circle-pad input, checks a held self-link, removes the override to recover StreetPass, and activates Vault with native A.

While open the editor owns pointer and keyboard; it releases the aim capture and blocks the recapture path, so opening it during play does not leave the cursor held by the game. **Save layout** writes both Scripted Menu placements and Pause rules to `menu-layout.toml` in the data directory; the next start with `--menu-editor` restores both. Existing layers-only files still load. Loading a console save state clears recorded drawing and pointer state but retains host editor placements and Pause rules, including unsaved edits. Other launches ignore the layout file. A console save state and **Save layout** are separate: the former restores the game; the latter persists editor geometry.

To make a change part of the game by default, apply the exported Rust/change request to the committed descriptor. Export writes `editor-layers-export.txt` with layer geometry, visibility and change requests, and the Pause panel writes `menu-layout-export.txt` with a Rust `PauseRules` fragment. The committed descriptor remains the source of truth for normal launches. Still to come: grouping and aspect previews.

Scripted Menu placements are kept for a 400-wide top screen and reseated for the current width. Wider, a box keeps its share of the room between the left and right edges (a box overhanging an edge keeps its gap to that edge; one spanning the screen stretches); narrower, it shrinks evenly and keeps its share of the room vertically. A box dragged at any width is converted back, so it is exactly where it was dropped at that width. Older saved placements are read as 400-wide; one made at a wider window and overhanging the right edge moves right by the extra width and needs re-placing.
