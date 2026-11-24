---
name: split-top-screen
description: Split a Scripted Menu's top-screen Details card into independently editable layers (paper, cards, emblem, weapon...). Use when the user asks to split, separate or break apart the components of a top screen / top view / Details card on a Screen.
---

# Split a top screen into layers

A Screen's top picture is split by **mesh node ownership**: a `TopLayer` table
`(layer name, model, nodes)` on the Screen's `LayoutKind::Board` row in
`crates/zakuro-core/src/aim_mouse/screen_transform.rs`. No other
code changes are needed per Screen. Reference tables: `GEAR_TOP`, `POWERS_TOP`,
`WEAPONS_TOP`. An empty node list owns the rest of that model.

Paths below: `K=tools/kiutrace/target/release/kiutrace`, ROM `dumps/kiu.cci`,
`S=<scratchpad>`, `G=.claude/skills/split-top-screen/scripts/pnm_grid.py`.
Shots are `.ppm`; view them with `python3 $G <out> <dest.png> <cols> <shots...>`
(stdlib only; Pillow is not installed and may not be installed).

## Steps

1. **Pick the save state** in `trace/` for the Screen (`ls trace | grep -i <name>`)
   and its registry index (the `scripted!(N, ...)` row). Only `Board` rows take
   `top_layers`; another kind must become `Board` first.

2. **List the models and nodes.** Rebuild kiutrace (`cd tools/kiutrace && cargo
   build --release`), then run a script `load <state>` / `menutop` / `run 4`
   with `RUST_LOG=info KIUTRACE_MENU_MODELS=1`. Grep `model layer`. Top models
   are `...Up....` (scene layers below F); `...Bt....` are the bottom Board.

3. **Probe every node.** Temporarily set the table to one row per node of every
   top model: `("0121:NN_U0000", "MenEquipUp0121", &["NN_U0000"])`. Rebuild,
   run `layers` to get indices (layers start right after `Details card`).

4. **Identify by moving, not hiding.** For each index: `load` / `menutop` /
   `run 6` / `layer <i> 240 120 <w> <h>` (w,h = the printed default) / `run 4` /
   `shot n<i>`. Hide-diffs lie: spinning weapons and duplicate card bodies make
   every diff look alike. **Placements persist across `load` within one run**,
   so in a single script each shot is cumulative (read it as "what this index
   added"); use separate runs for clean singles. Name each part from the sheet.

5. **Own every top model.** Write the final table back to front. Any node left to
   the Details card draws *after* the layers and covers them (Weapons: the
   paper model and a second set of card bodies blanked everything). Nodes that
   draw nothing can stay unowned. Use `GLOSSARY.md` terms for names.

6. **Verify** in one script from the state: `layers`; `shot default`; move 3-4
   layers apart with `layer`; `shot moved`; `visible <paper> 0`; `shot nopaper`.
   Check: default matches the unsplit screen (compare words at `--crop
   125,0,150,80 --zoom 3` against a run with `top_layers: &[]`), each part moves
   alone, and hiding the paper keeps the words. Also reshoot Gear
   (`trace/armory-gear.state`) and Powers (`trace/poweredit.state`).

7. **Test and ship.** `cargo test -p zakuro-core --release`
   (note pre-existing failures by rerunning without your change). Windows:
   `cargo build --release --target x86_64-pc-windows-gnu -p zakuro`, copy
   `target/x86_64-pc-windows-gnu/release/zakuro.exe` to
   `dist/zakuro-kiu-mouse.exe` and `/mnt/c/Users/Patrick/Downloads/zakuro-kiu/zakuro-kiu.exe`,
   compare `sha256sum`.

8. **Record** the node map in `kiu/analysis/FINDINGS.md` (next to "Gear top-screen
   node ownership") and the layer list with indices in
   `docs/agents/screen-transform.md` (next to the Armory — Gear/Powers paragraphs).
   Add anything new you learned to this file.

## Bottom screens

The same table splits the bottom Board: `bottom_layers` on the `Board` row,
each layer named in `group` with its native box (Idols: `IDOLS_BOTTOM`).
- Back/AR and masks often sit in layers 0x10..0x12; the model log covers them.
- A node layer replays only its owner model; replaying every model per layer
  exhausted the drawing buffer (black frame). Same for text: list which node
  layers carry words in `board_text_now`, or a full-screen box replays every
  Row's glyphs once per layer.
- Dark words vanish when a layer is probed alone on black; keep the Background
  layer visible, or judge text by `RUST_LOG=...menu_top=debug` "board glyph".
- In this harness `hover` takes native bottom points; use `mhover` (top
  pixels through the Placements) to test clicks on moved layers, and
  `KIUTRACE_TOUCH=1` prints the touch each `press` sends.
- Some saved states reload with a black top (Idols' List); navigate there
  in-script from a neighbouring state instead.
- Convert shots with `ffmpeg -i x_top.ppm x.jpg` if the PNG grid is rejected.

- A Screen with no registry row needs one first: find its object (scan the
  heap for a word near a sibling's vtable), then its update with `entered
  <object> 2` (the first function entered with r0 = the object, from the
  menu dispatcher). Add the `Screen` variant, `SCRIPTED_UPDATES`, `PLACES`
  and `SCREEN_MENUS` (`menuid` prints the MENUID) entries.
- A common backdrop (`MenCmnBg0000`) left as a plain group model draws over
  the whole top screen; give it a bottom node layer.
- Bottom node layers are cut at their group boxes: probe with full-screen
  boxes, or a part outside its box looks like it draws nothing.
- Words need a node layer that `board_text_now` lets replay text; a node
  that draws nothing (`pPlane1`) can own such a layer.
- Owning each of many small nodes (portrait pieces) separately can lose
  them; try one rest layer.

- A `Generic` Screen made a Board needs its bottom split too: a single
  model-less bottom part broke mouse clicks on Fuse Weapons. Test clicks
  with `mhover` + `press`/`release` (`mpress` and `click` did not act on
  Fuse in any build).
- Count the replays. Fuse froze in a loading at 17 bottom node layers and
  not at 16. Layers that only draw on a page (dialogs) can be made
  unavailable outside it (`available`, from a game phase word), which also
  keeps them from taking the pointer. Replay the Screen's whole flow,
  including what follows a dialog, on the final build.
- Find a Screen's phase by diffing heap dumps between pages (small words
  steady within a page), then `watch` + `trace` with
  `KIUTRACE_WATCH_REGS=1` for its writer and base register.
- A particle model (`PtcCmn*`) has no nodes; replays drop it from the layer
  list unless they own it. If a probe shows the same sparkles in every
  layer, including one that owns nothing, look for such a model.
- Nodes that draw nothing in the idle Screen may draw in its animation
  (Fuse's flash rays): replay the Screen's animation before leaving them
  to the Details card.
- A bar under the hint's words can follow the hint: add its layer name to
  `HINT_BACKING`.
- Old `trace/` states (0.2.30) no longer load; the user's desktop slots are
  in `/mnt/c/Users/Patrick/AppData/Roaming/zakuro/states/0004000000030100/`
  ("state N" means `N.state`).

- Boxes probed on one page of a multi-page Screen can cut another page's
  bigger parts (Chapter Results: a Power's bar vs a weapon's stats card).
  Probe every page the Screen shows. Adding a layer renumbers every later
  part, so renumber that Screen's `part`, `parts` and `hidden` entries in
  `kiu/menu-layout.toml` and the user's desktop `menu-layout.toml`.

- Bottom node layers of a Screen in `UNCUT_BOARDS` are drawn whole (ADR
  0008); there the box is only the frame and the word anchor area. kiutrace
  `rects` prints each bottom node layer's projected mesh boxes: a guide for
  writing boxes, padded by transparent margins and thrown off by invisible
  or off-screen meshes. Moving a Screen into `UNCUT_BOARDS`: compare
  shots against the old build (copy `target/release/kiutrace` first) on
  every page, scrolled; a list that overflows its window goes in
  `LIST_WINDOWS`.

## Draws that are not models

Something drawn in every layer's replay that no node or model flag hides
(the Idols' floor shadow) is not a model: find its draw by binding. Blank a
texture by hash in the software raster to see every copy, then watch the
command-list words that bind it (`KIUTRACE_WATCH_REGS=1` prints the
writer's registers and stack) to reach the draw function, and give it a
pseudo-owner hooked like `CIRCLE_SHADOW` (`menu_top::circle_shadow`).

## Known pitfalls (already fixed in `menu_top.rs`)

- With layers, the native pass skips the top words and the Details card must
  replay last; `MenuTop::part` used to drop the card, so its words vanished.
- The card's replay uses the guard-band projection; the native viewport
  mapping squeezed the words to half width.
- `expectmenu` needs an argument; hovering Back with `hover`/`press` did not
  activate it in this harness on Armory — Weapons, with or without layers.
