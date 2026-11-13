# Kid Icarus: Uprising on PC

The goal is to play Kid Icarus: Uprising (USA, 0004000000030100) on a PC with a
mouse and keyboard, on one screen of any shape, without changing how the game
plays.

It is not a standalone native port. The game runs in a fork of the Zakuro 3DS
emulator, with its code recompiled ahead of time by 3dsrecomp. A few functions
are hooked so the fork can change what they do; everything else runs as the
game wrote it. Each hook is checked against unhooked play over the same frames,
so gameplay stays the same.

## What works today

### Mouse and keyboard

- **Air battles**: the mouse aims where you point, absolutely.
- **Ground battles**: the mouse turns the camera, relative like a PC shooter.
- **Exo Tank**: mouse look remains active when the vehicle disables the ordinary ground camera.
- **Action Bindings**: keys, mouse buttons and controller buttons are bound
  to what you do (Attack, Use Power, Move, Pause, Confirm, Back…), each
  with a primary and secondary input per device. Defaults: WASD to move,
  left click or Space to attack, E or right click to use the current power,
  the arrow keys for the power list in battle and for menus outside it, V
  for the game's Change View, F and G for the vehicle's ability and exit,
  Escape to pause in battle and
  as Back in a Screen. On a controller: left stick, right stick, RT attack,
  RB power, Y/△ Change View, the d-pad, A/B, Start, stick clicks for the
  vehicle. An input
  acts only in the context it was pressed in, so Escape held into the Pause
  it opened does not close it.
- **Gyro aim** (optional, off by default): a DualShock 4, DualSense or
  Switch controller's gyroscope feeds the console's, Always On in battle or
  held with LT; sensitivity, inversion and calibration. The game aims with
  it only with the C-Stick & Gyro patch installed.
- No touch screen involved.

### One screen

Everything you need in battle is drawn on the top screen by the game's own UI
code, so it stays sharp at any resolution:

- narration and subtitles
- character portraits
- dialogue illustrations and boss cards
- hearts, item box, power icon, held item and pickup popups
- the pause menu, Options and its pages, and the dialogs over them

The bottom screen keeps its native drawing and touch controls. Menus that
support the mouse move onto the top screen too, with proportional artwork,
grouped lists and compact hints.

### Widescreen at any shape

- A wider view, not a stretched one, at 16:9, 21:9, 32:9, or a window dragged
  to any size.
- No pop-in at the new edges; the HUD keeps its shape and sits by the window's
  edges.
- Aiming is correct across the whole width.

### Menus with a mouse

- The option under the pointer lights; a click uses it; over nothing, nothing
  is lit.
- Keys and gamepads get the game's own cursor back.
- Works in the pause menu, Options, dialogs, file select, chapter select, the
  main menu, the Solo hub, Arms Altar, Practice Range, Idol Toss, Treasure
  Hunt, the Vault, the Armory, and the Fiend's Cauldron.
- Armory lists scroll with the wheel; Shift + wheel scrolls horizontally.
- Chapter select shows its chapters in one row across the window; the wheel
  scrolls it without changing the chapter picked.
- Click-and-hold and dragging stay native touch.

### Performance

- 60 FPS patch included (`dist/60fps/code.ips`).
- Smaller HUD and reticle.
- CPU overclock for heavy scenes.
- Land battles run at full speed.

## Known limits

- Online (Together) mode is out of scope for now.
- Windows mouse testing is still an acceptance check; the headless checks pass.
- Scripted menu overlays need the 3D slider off, and dense stereo menu boards
  fall back to native rendering.
- A few controls still need a held touch: Control Configuration and the
  Air/Land Battle sliders. The Music Gallery list does not take the wheel yet.
- The Fiend's Cauldron's bottom ring background and hearts-box frame are still
  being restored.
- Boss coverage: natural boss defeat, and bosses beyond Medusa, Hewdraw and
  Twinbellows, still need checks.
- The arrow keys and d-pad reach the game's own power list in battle (Down
  shows it); moving through it and using items from the keyboard still
  need checking.
- KIU's own Options Screens don't show the PC Preferences yet. Until they
  do, F12 opens the Frontend Menu, whose Settings window has a Kid Icarus
  section for the bindings, sensitivities, gyro and HUD/portrait sizes.
  The CPU overclock is still `ZAKURO_OVERCLOCK`.
- Dialog text can occasionally stack in some scenes; the cause is known and
  the fix is planned.

## Running it

You need a decrypted dump of the game. Put it in `dumps/`; never commit or
share it.

Build and run the fork with the game:

```
cargo run --release --manifest-path tools/zakuro/Cargo.toml -p zakuro -- dumps/kiu.cci
```

Recompile the game once from Zakuro's library (the Recompile button) so it runs
at full speed. F12 (or a controller's Home button) opens the menu over the
game, where the sound and graphics settings live; in Kid Icarus Escape is the
game's own Pause and Back.

The one-screen features can be turned off for comparison with the original
layout: `ZAKURO_MENUS_TOP=0`, `ZAKURO_NARRATION_TOP=0`, `ZAKURO_MIRROR_BOTTOM=0`,
`ZAKURO_PAUSE_TOP=0`, `ZAKURO_CAULDRON_TOP=0`, `ZAKURO_MENU_HOVER=0`.

## Where to look

- `PLAN.md`: the status of each feature and the work left.
- `analysis/FINDINGS.md`: how the game works, and the addresses behind each
  hook.
- `docs/adr/`: the main design decisions.
- `GLOSSARY.md`: the project's terms.

## Layout

- `dumps/`: your decrypted dump goes here. Never commit or share it.
- `tools/zakuro`: the Zakuro fork, where the hooks and the frontend options
  live.
- `tools/3dsrecomp`: the recompiler, with its patch for Zakuro's ABI.
- `tools/kiutrace`: headless harness for testing scenes from save states
  (input, screenshots, memory, tracing, and each hook on its own).
- `tools/funclist`: exports 3dsrecomp's function list for Ghidra.
- `ghidra/`: headless import, decompile and disassembly scripts for code.bin.
- `scripts/`: float scans, the 60 FPS IPS builder, code diffs, screenshot
  conversion.
- `overrides/`: game functions recovered as C Overrides, built with
  `3dsrecomp build --overrides overrides/`; `docs/decomp/LEDGER.md` counts
  them and records how each was checked.
- `analysis/`, `trace/`, `build/`, `dist/`: generated output, kept out of git
  because it holds game code or binaries.
