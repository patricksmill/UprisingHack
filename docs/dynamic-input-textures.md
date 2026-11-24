# Dynamic Input Textures

Some of the game's button prompts are not text. They are pictures in its
textures: the round A, B, X and Y and the D-pad and Circle Pad icons on the
battle HUD atlas, the console diagrams on the Control Configuration Screen,
the X and Y pills on Scripted Menus. The font's button symbols already follow
the Action Bindings (`crates/zakuro-core/src/services/prompts.rs`), but these
pictures do not. Dynamic Input Textures redraw them with the player's own
keys, mouse buttons or controller buttons, the way Dolphin's feature of the
same name does for GameCube and Wii games.

## How Dolphin does it

Dolphin (`docs/DynamicInputTextures.md`,
`Source/Core/InputCommon/DynamicInputTextures/` in the Dolphin repository)
reads JSON files from `Load/DynamicInputTextures/<game id>/`. Each file lists,
under `output_textures`, the custom-texture name of a game texture
(`tex1_<w>x<h>_<hash>_<format>.png`). For that texture it gives:

- `image`: the base picture, usually a redrawn or upscaled copy of the
  game's texture;
- `emulated_controls`: for each emulated controller (`Wiimote1`, `GCPad1`)
  and control (`Buttons/A`), the regions `[left, top, right, bottom]` of
  `image` where that button is drawn;
- `host_controls` (per texture) or `default_host_controls` (per file): for
  each host device (`DInput/0/Keyboard Mouse`, `XInput/0/Gamepad`, or `""`
  for any) and host key, the picture of that key.

When the bindings change, Dolphin reads which host key each emulated control
is bound to. It copies `image`, fits each bound key's picture into that
control's regions (nearest-neighbour, keeping proportions unless
`preserve_aspect_ratio` is false), and hands the result to its custom texture
loader under the same name. A control whose key has no picture keeps the
original pixels. `specification` is 1, or absent.

## How Zakuro does it

Zakuro reads the same format (`crates/zakuro-core/src/dynamic_textures.rs`)
and draws the results in memory (`crates/zakuro-gpu/src/dynamic.rs`), in
front of the texture pack. Each time the button symbols change (the player
switches between keyboard and controller, or rebinds), the patches are made
again and the GPU's textures re-pick their pictures without being decoded
again. Like texture packs, they need **Texture packs** on and the 3D drawn on
the GPU.

The 3DS has one controller, so `emulated_controls` takes any device name
(`"3DS"`), and the controls are the console's inputs: `A`, `B`, `X`, `Y`,
`L`, `R`, `ZL`, `ZR`, `Home`, `DPad`, `Up`, `Down`, `Left`, `Right`,
`UpDown`, `LeftRight` and `CirclePad`. Dolphin-style names such as
`Buttons/A` and `D-Pad/Up` also work. Host devices are `Keyboard Mouse` and
`Controller` (Dolphin's `DInput/...`/`XInput/...` names map to those), or `""`
for either. Host keys are named as the button symbols label them: a key
cap's text (`Space`, `Esc`, `E`, `↑`, `WASD` for the movement keys
together), `Left Click`, `Right Click`, `Middle Click`, a controller's `A`,
`LB` or `○`, `Left Stick`.

Zakuro needs less than Dolphin:

- **No `image`**: the base is the game's own texture as the GPU decodes it,
  `scale` times its size (4 by default), and regions are in the game
  texture's texels. A config then carries no picture from the game, only
  hashes and rectangles.
- **No key pictures**: a key without a `host_controls` picture is drawn: a
  keyboard key as its name or symbol alone (`E`, `Esc`, `↑`), a mouse with
  its button lit, or a controller's button as the font's symbols draw it.
- **Actions**: a control that is not a console input names one of the
  game's actions as the frontend passes them (`NextIdol`, `ZoomIn`,
  `IdolViewOrbit`, `IdolViewPan`), and draws whatever that action is bound
  to on the device in use. An action bound to nothing keeps the game's
  picture.
- **Styles**: a region can be an object `{"rect": [...], ...}` with
  `fill` (a colour painted over the region first, `"#RRGGBBAA"`),
  `fill_shape` (`rect` or `ellipse`), `blend` (`replace`, the default, or
  `over`), `tint` (the drawn symbol's colour) and `outline` (a colour around
  it, or `null`). The same keys on a texture set its regions' defaults.

The game's picture stays wherever the console's own buttons are what the
player uses, or an input's symbol is the console's (a Nintendo-layout
controller's A bound to A).

## Where configs come from

1. Zakuro's own for Kid Icarus: Uprising,
   [`kiu/dynamic_input_textures/kiu.json`](../kiu/dynamic_input_textures/kiu.json),
   built into the executable.
2. Every `.json` under the game's mods folder, in
   `mods/0004000000030100/dynamic_input_textures/`, with its pictures beside
   it. A texture named here replaces Zakuro's entry for it.

## Writing a config

Commands run from `kiu/`.

1. Dump the RomFS textures with their pack names:

   ```bash
   python3 scripts/kiu_textures.py dumps/kiu.cci build/textures/menu "" us/menu/
   ```

   Each `<name>.png` gets a `<name>.txt` whose `pack` line is the name the
   config keys on (`tex1_256x256_B76149565D0DDB15_9_mip0.png`). The PNGs are
   in the same orientation and texel grid the regions use.
   `ZAKURO_DUMP_TEXTURES=<folder>` dumps what the running game draws
   instead, under the same names.
2. Measure each button's rectangle in the dumped PNG and add it under the
   texture's `emulated_controls`.
3. Preview the result with sample bindings without starting the game:

   ```bash
   ../target/release/ditpreview - build/textures/menu out/dit keyboard
   ```

   `-` reads Zakuro's built-in config; give a folder to read the configs in
   it. `controller` previews a PlayStation pad's symbols.
4. Check it in the game: `kiutrace`'s `prompts keyboard|controller|console`
   switches the symbols mid-script (with `KIUTRACE_SCALE=2` for the GPU).

## The game's input pictures

| Texture | Pack name | Where | In the config |
| --- | --- | --- | --- |
| `gm_00` (`us/info/common.zrc`) | `tex1_256x256_B76149565D0DDB15_9` | battle HUD atlas | A, B, X, Y rounds, D-pad, Circle Pad |
| `gm_02` | `tex1_256x256_964D19A6061EDBC1_9` | battle HUD atlas | the R/L bar's letters |
| `gc_01` | `tex1_256x256_72981D8DE1C120AC_13` | Control Configuration diagrams | Circle Pad and D-pad of the first diagram |
| `MenOptionButtonUp0000` | `tex1_128x64_45112B0A56091434_9` | Options buttons | A, B, X, Y, and the L/R bar |
| `MenPresentSeedUp0010`, `0020` | `tex1_64x32_592391B19BBEB521_13`, `tex1_64x32_9C6652BC1AFF1581_13` | X and Y pills | X, Y |
| `MenOnlineTopBt0020` | `tex1_64x32_16632EE62136A3EE_13` | X pill | X |
| `MenAltersoldBt0100` | `tex1_128x64_46C24D45ABB8D701_4` | Y button | Y |
| `MenColViewerBt0008` | `tex1_128x128_7094C30E0DFF73BC_13` | Idols: ◀ L R ▶ | PreviousIdol, NextIdol |
| `MenColBt_Lang0003` | `tex1_256x32_AB0EE5FCDF1DAEC8_13` | Idols hint strip | IdolViewOrbit (Rotate), IdolViewPan (Move), ZoomIn, ZoomOut |

Not configured yet: `gm_00`'s small `ABXYLR` letters and ABXY cluster,
`gc_01`'s other diagrams (face buttons, shoulders, stylus), `3ds_tex02`–`04`
(console outlines), and the menu banners with `(START)` baked into their
lettering (`MenBetheartBtLang0000`, `MenOnEq01Bt_Lang0022`/`0023`,
`MenEquip02MrclBt_Lang0001`, `MenAltersoldUp_Lang0000`). START has no input
in the button symbols yet.
