# Findings: Kid Icarus: Uprising (USA, CTR-P-AKDE, 0004000000030100)

Addresses are for this dump's `code.bin`, loaded at 0x00100000. Internal
title name "apollo". No CRO modules: all code is in the executable.

## Layout

| Segment | Address | Size |
| --- | --- | --- |
| text | 0x00100000 | 0xB950F0 |
| rodata | 0x00C96000 | 0x9B990 |
| data | 0x00D32000 | 0x31C78 |
| bss | after data | 0x1D20B0 |

3dsrecomp: 49,236 functions, 93.6% of text is code, 2.2% unreached.
Ghidra's own analysis found 28,208; `ghidra/ImportFunctions.java` adds the
rest from `analysis/functions.txt` (exported by `tools/funclist`).

## Projection path (widescreen)

| Address | What | Confidence |
| --- | --- | --- |
| 0x003901C4 | Camera projection getter. Lazily builds the pivoted (screen-rotated) projection at `camera+0x6C` (16 floats), sets flags `0x44` at `camera+8`. With no lens object at `camera+4` it uses the default block at 0x00D4E248 {near 0.1, far 1000, fovy 60°, aspect 4/3}; otherwise calls the lens's virtual at vtable+8 to fill the matrix. | high |
| 0x00474BB8 | Trig routine (sin/cos/tan family, 2/π range reduction). Called as tan(fovy/2). | high |
| 0x008D108C | `Frustum(m, l, r, b, t, n, f)` in s0..s5: builds a 4×4 frustum, m[14] = -1. | high |
| 0x008D0C38 | Pivot a projection for the rotated LCD (modes 0–4). | high |
| 0x008D11B0 | Stereo camera: from a base projection and the 3D slider, builds left/right eye projections with Frustum + Pivot. Matches the SDK's ulcd StereoCamera. | high |
| 0x00916234 | Per-frame camera submit: copies view (3×4) and projection (4×4), un-pivots the projection and passes it to 0x008D11B0. | high |
| 0x00911288 | Calls 0x00916234 with the camera's view and `FUN_003901C4(cam)`. | high |

The pivoted projection has the horizontal (400 px) axis in row 1:
`m[1] = cot(fovy/2)`, `m[4] = -cot(fovy/2) / aspect`. Widening the view
means lowering |m[4]| (or raising aspect) before the stereo code runs.

The literal 400/240 (5 copies) is **not** the projection: the three
functions that read it are animation timers (t × 1/0.6 s).

Battle camera, from state 1: 0x00E14BEC, its lens at +4 0x00E14BB4
(perspective, vtable 0x00D20080): near 3.5 (+8), far 40000 (+0xC), fovy
0.785 (+0x10), aspect 1.6667 (+0x14). Two orthographic lenses (vtable
0x00D200B0) sit at 0x00E14BCC (top, -200..200 x -120..120) and 0x00E14DAC
(bottom, -160..160). Nothing writes the aspect during play; writing it and
clearing bit 2 of camera+8 rebuilds the projection with it, and the view
widens (culling and the aim ray with it). Callers of 0x003901C4 each
frame: 0x00360168 (aim ray, twice), 0x007C48EC (reticle), 0x00911698 and
0x00911304 (render submit). The ground turn from the reticle scales with
atan(tan(fovy/2) x aspect).

## Game camera fields

`FUN_0042F534(mgr, 0, 1)` returns the active game camera. Seen in
0x00756754 (visibility bounds): fovy at `+0x3C`, aspect at `+0x40`.
That function expands a box by tan(fovy/2) × dist × aspect, so it is
culling-related and needs widening along with the projection.

## Input services present

`hid:USER` (touch, buttons), `ir:USER` (Circle Pad Pro), `cam:u` (AR
cards). The last two are possible gaps in Zakuro.

## Darkshade's C-Stick & Gyro patch (USA v172), as a map

`patch/` holds it. It replaces code.bin outright and appends 0x1F2000 bytes
of new code at 0x00D64000+ (exheader enlarged). `scripts/diff_code.py`
diffs; `analysis/mod/` has the diffs and a disassembly of the widescreen
code (`w169_disasm.txt`). 16:9 vs 21:9 differ only in the scale constant
(0.9375 vs 0.714 = native 5:3 ÷ target aspect) at 0xF52168 / 0xF52410.

Widescreen hooks (on top of the base patch):

| Hook | Original | Mod code | What it does |
| --- | --- | --- | --- |
| 0x003901EC | `pop {r4,r5,pc}` at end of projection getter | 0xF52000 | If the returned matrix is a sane perspective (m[14] = -1, m[15] = 0, all finite), copy it to a 32-slot ring and scale row 1 (m[4..7], horizontal) by the constant; return the copy. Confirms 0x003901C4 is *the* projection hook. |
| 0x0092F408 | entry of 2D sprite-batch flush | 0xF52430 → 0xF52174 | Only for 14 whitelisted callers (table at 0xF52424; six stubs 0xF52468.. pass the call site): squeezes each quad's x (stride 0x60 per quad, 4 verts × 0x18) about the centre so HUD isn't stretched; edge-anchored elements handled separately (160.0 constant). |
| 0x006D15B4, 0x006D1764 | returns of 0x006D14F0 (reticle bounds) | 0xF52568 → 0xF524C8 | Re-centres reticle bounds on the wider screen using aim+0xE0/+0xE8. |
| 0x006D26C0 | `bl 0x00B12218` in 0x006D269C | 0xF526F8 → 0xF525BC | Corrects the turn amount from reticle x for the wider view. |

Base-patch hooks worth knowing (all in `analysis/mod/base_hooks.txt`):
0x006D2E30–0x006D2F3C wrap the aim update calls; 0x006D1E20/0x006D2050 in
0x006D1FF0; 0x006D27DC; 0x006D2B5C–0x006D2BE4 in 0x006D2ABC; 0x006CF460,
0x006CF7DC; 0x001DCE2C–0x00214834 and 0x0024C270–0x00257F50 (input and
control-settings code, to be read).

## Aim (player reticle) controller

Per-frame update `0x006D2D8C(aim)`:
1. vtable+0x2C(aim): likely the input read (touch/stick → reticle). **To identify.**
2. `0x006D14F0`: reticle free-movement box from aim+0xE4 (half width?),
   +0xEC (half height?) and the state at +0x81 (ground/air?).
3. `0x006D1FF0(aim, out, cam)`: auto-centre / lock-on modes 1–4 at aim+0x208.
4. `0x006D269C(aim, out, cam)`: applies camera turn.
5. `0x006D2ABC`, `0x006D2820`.

Reticle position: aim+0xA4 (x), aim+0xA8 (y). aim+0xD5 combines non-local
ownership with camera disable; it is not an ownership-only flag. At
`0x006D2E18`, r0 is zero for a local controller and r4 is the aim object.
`0x006D2E2C` then forces +0xD5 to one when camera+0x179 is zero. State 2's
Exo Tank takes that path, so checking +0xD5 discarded its mouse motion.
The mouse hooks now retain the ownership decision before the camera override.
State 2 replay verified horizontal/vertical mouse response and Circle Pad
movement; the regression covers local vehicle input and remote rejection.
Game camera (`aim+0x8C → +0x2C`): fovy at +0x2B8, aspect at +0x2BC.

Vehicle actions share the battle power panel (`UI+0x140 → context+0xB64`).
Fresh State 2 touch/release at (30,220) writes command bit `0x4` at panel+8
from `0x00A12F88`; (85,220) writes `0x8` from `0x00A12FA0`. The battle
command consumer `0x00A30D00` routes them to `0x006E88C8` (ability) and
`0x006E7DF8` (exit). The native hit-test `0x00A171B4` requires panel input
enabled (first word bit `0x100000` clear) and vehicle-control animation
panel+0xF0 at least float bits `0x3F7EFEFF`. The binding request expires
after three frames and is consumed once, preserving other command bits.
State 2 smoke reached the native ability request store at `0x006E891C`
and showed the activated effect and completed dismount after 40 frames.
`kiutrace` commands `vehicle ability` / `vehicle exit` exercise this path.

Ground camera pitch (`trace/s8.state`, found 5 November by tracing writes and
disassembling with capstone): the game camera's matrix object is at
camera+0x278 with yaw at +0x34 and pitch at +0x38 (camera+0x2B0, radians,
negative up). `0x007462A4` (called once, from the camera update 0x00703B68)
turns the pitch and clamps it: s18 (down) and s19 (up) are loaded from a
parameter block (`[sl]+8`, `+0x14`), adjusted by the ground's slope, capped
at ±80° (0x00746650/54) and final at 0x0074651C, where r6 is the aim object.
Here they are 15° down and 12° up. With the reticle in its box, aim reach is
+26°/−20.8°; with it held in the middle, +16.2°/−10.9°, the box's 50 of 120
pixels being 9.8° of a 45° view. The port widens both limits by 10° while
the mouse aims on the ground (`PITCH_LIMITS`, `CAPTURED_PITCH_REACH`).
`0x0034C210` is clamp(s0, s1, s2); `0x0034C248` wraps an angle.

Ground camera distance (`trace/s8.state`, found 20 November by poking and
watching writes): the camera update `0x00703B68` stores its camera in
`*0x00D3A18C` (0x141FDE68 there). The matrix object at camera+0x278 holds the eye at
+0, the direction at +0xC, the target at +0x24, the distance at +0x30 (34 on
the ground), the yaw at +0x34, the pitch at +0x38, the fovy at +0x40 and the aspect at +0x44.
`0x006063E8(matrix, yaw, pitch)` places the eye at target − direction ×
distance. `0x007EF350` (called only at 0x00703FAC) works the distance out
again each frame from a base at camera+0x6AC times a zoom at camera+0x6D8
(1, eased back after a poke by 0x0090AECC), floored at 0.01, and stores it
at `0x007EF464` (`vstr s0, [r4, #0x30]`, r4 the matrix object). r10 there is still the
caller's aim object (vtable 0x00CF1B94, +0x81 = 1 on the ground). A poke
of +0x30 lasts one frame. The port scales s0 at that store
(`CAMERA_DISTANCE`, the PC Preference "Land Battle camera distance",
default 1.25): 42.5 held steady through 175 frames of running.
Against a wall the collision (`0x0060634C`, from 0x00772490) draws the eye
in and writes +0x30 again; then `0x007724B4` divides that by the distance
before the zoom (`[r6+0x28]`) and eases the zoom at r6+0x2C to the ratio, which
multiplies the next frame's distance. Divided by the game's own distance,
that ratio carried the port's scale, so the scale compounded every frame
against a wall (state 1 of 23 November at 1.6: 54, 83, 128 ... 6470 in 17
frames). The port divides by the scaled distance there
(`CAMERA_ZOOM_RATIO`): the same pan now draws in to 14 and eases back to 54.

Pit's ground movement (`trace/s8.state`, measured 19 November by stepping
the Circle Pad and reading Pit's position each frame): his position is the
vec3 at `0x141FBEA0` (x, y, z; many matrix copies nearby). The game's input
manager reads the stick through the SDK pad reader (`0x0012D150`: clamp 40
to 145, then -100..100 and floats ×0.01), so host lean is not the game's.
Speeds below are units a frame, host lean as Zakuro sends it (×150 raw):

| Stick | Settles at |
| --- | --- |
| 0.3 / 0.5 / 0.6 / 0.7 / 0.8 / 0.85 held from rest | 0.19 / 0.34 / 0.42 / 0.49 / 0.78 / 0.87, gaining 0.05 a frame |
| 1.0 reached in 6 frames or more | 1.00 (run) |
| 0.88 or more reached within 5 frames of rest | dash: 2.72, easing to a 1.60 sprint held at full lean |
| from a held 0.5 or 0.7 straight to 1.0 | 1.00 (run), no dash |

A movement key's instant full lean always dashes; diagonals lean per
axis, so 0.6/0.6 (length 0.85) behaves as 0.85. The Walk action
shortens the keys' vector to the Walk speed preference
(`Movement::walk_lean`, default 0.5, kept within 0.3..0.8 so Pit moves past
the dead zone and stays short of a dash).
`trace/` states are Zakuro 0.2.30 and do not load in 0.2.32; these runs
used a probe built against the 0.2.30 tree.

## Still to find

- the lens virtual that fills the projection (camera+4, vtable+8)
- where the game camera's aspect (+0x40) is set
- touch → reticle / camera handler
- HUD 2D projection

## Frame rate (60 FPS patch: `dist/60fps/code.ips`, `scripts/make_60fps_ips.py`)

- Vsync manager singleton (`FUN_003B6FE4`, global 0x00D63824): +0x50 frame
  rate (30), +0x54 swap interval (2), set once in 0x0048A794. Present/wait in
  0x0091038C: waits until interval + leftover vblanks passed; leftover
  (+0xA8) kept as parity `& 1` at 0x0091041C, which at interval 1 forces a
  2-1-1 pattern (40 FPS). +0xB0 = vblanks the last present covered.
- Update pacing 0x00828E34: updates = leftover + vblanks >> 1
  (0x00828E8C); 0x00828D68 sets the frame-count singleton's (0x00E174E4)
  base frames +0x20 to the literal 2.0 at 0x00828E30 each update.
- `ActorTaskUpdate` (0x00A3DB84) turns frames into the frame info object
  (+0xC dt = frames/60, +0x18 frames) that every actor tick receives.
- Patched: 60/60 frames drawn on the old-3DS cycle budget (no overclock
  needed); strafing and firing for 1.5 s matches the unpatched game frame
  for frame.

## Training HUD

Air and land Practice Range both flush the defeat and distance counters at
`0x0060D66C`: one batch of 11 quads spanning both bottom corners. It is
separate from the normal battle HUD callers. The generic widescreen fallback
shrunk each background slice around its own centre, opening gaps, and did
not apply the HUD scale. `place_training_hud` now scales every slice and
digit around its panel's bottom corner (0,240 or 400,240), with widescreen
correction on x. Headless screenshots checked in both modes at 16:9 and
21:9 with HUD scale 0.7; a unit test checks shared slice edges stay joined.

## Dialogue illustrations and boss panels (October 31)

The current desktop slot 1 is the Chapter 3 air scene from the Hewdraw
screenshot. Run 60 refreshes from it to reach "Like, this guy?" and the blue
sprite. The saved fixture is `trace/ui-probe/hewdraw-blue.state`.

- `0x00A17F70` draws the battle's bottom UI. The illustration branch calls
  `0x00A0F734` at `0x00A18244`, handing it the bottom UI, 2D context, pass
  number and fade. It draws a black center band (`0x00A0F814`), zero to four
  illustration sprites (`0x00A0F9E8`), then black bands/borders (return to
  `0x00A18248`). Only the illustration flush is mirrored.
- The bottom UI's `+0x200` is the image-slot mask. Four `0x24`-byte slots
  start at `+0x204`: image id/flags, x/y at +4/+8, UV offsets at +0xC/+0x10,
  width/height at +0x14/+0x18, scale at +0x1C, alpha at +0x20. Hewdraw's slot
  is x 112, y 60, width 24, height 31, scale 4: its quad is 96 x 124, not
  the entire 320 x 130 band. Fit the union of active image rectangles to
  retain multi-sprite arrangements and the original aspect ratio.
- Hearts occupy y `(40..90) * hud_scale` on top. Images start at
  `98 * hud_scale`, end eight HUD pixels above the visible portrait artwork,
  and sit eight HUD pixels from the right edge. Pit's visible head starts
  about 64 pixels into the portrait's 256-pixel tile. Transparent tile
  padding is excluded from the reserved portrait height.
- `0x00A18204` calls `0x00A0E744`, drawing the boss artwork and baked-in
  title in two tiles, x 0..256 and 256..320, y 0..240. Reveal placement is
  128 x 96 HUD pixels, centered at y 40. Player-health visibility
  (`context+0x31BC & 1`) selects the combat layout: clip both tiles and UVs
  to y 0..64, then place the title at y 8, 160 x 32 HUD pixels. Preserve
  their common x transform so the texture seam stays joined.
- `0x00A18224` calls the boss overlay/effect drawing `0x00A14320`.
  `0x00A18130` calls the alternate text drawing `0x00A15C90`; its first
  full-screen black flush (`0x00A15D3C`) is omitted on top. Those paths use
  the boss region. No separate boss-health draw appeared in the inspected
  Twinbellows introduction/combat fixtures. Other bosses and natural defeat
  still need coverage.
- Extras now replay at `0x0060D66C`, after all top battle-HUD branches.
  `0x0060D60C` can be skipped during a reveal. The pass keeps its current
  context in r4; capture that rather than relying on a previously drawn
  score, especially across state loads. Bottom passes clear missing calls;
  recordings are valid for the next top UI pass, independently of the number
  of intervening vblanks. Retain calls for both eyes: the game submits the
  shared UI list to each. Allow twelve refreshes after loading a fixture so
  captures include a completed replay, not only the initial cold frame.
- `0x0060D66C` is also the native training-HUD flush return. Restrict its
  corner placement to native draws; otherwise a replayed narration line
  takes that branch and shifts right. A regression test exercises the real
  sprite hook with narration at this address across HUD scales/aspect ratios.
- Restore the recorded native material selectors around each replay and
  mark their bindings dirty. Suppress decorative bands by discarding their
  pending sprite batch, leaving dirty material state for the next visible
  draw, rather than submitting off-screen degenerate quads.
- Sprite flush flag `0x08` (context +0x50) also writes the GPU scissor
  (registers 0x65-0x67) from context +0xFC/+0x100, packed in framebuffer
  coordinates computed for the bottom screen. The power panel's highlight
  flush (`0x00A116B0`) sets one around the cards, so on the top screen the
  icons and counters after it were clipped away. Mirrored replays move that
  rectangle with the quads (`place_scissor`) and restore it on return.
- Chapter 4's rail (texture `0x04518780`, ETC1A4) is a geometry-shader tube:
  28 centreline vertices become 360 triangles. Its setemit instructions use
  the winding bit (22), which swaps a completed triangle's first two
  vertices; ignoring it culled half the outer faces and drew inner ones,
  giving the opaque, thorny look. The stage-4 combiner then gives
  `tex0.a * 0.898` alpha, i.e. the translucent tube seen on hardware.

Validation artifacts are under `trace/ui-probe`: twenty-four software layouts
(four scenes, three aspect ratios, two HUD scales), 2x hardware captures,
both stereo eyes, and isolated image/boss removal fixtures. All 23
`aim_mouse::tests` pass. After 180 identical firing refreshes per variant in
Hewdraw and Twinbellows combat, the player-health and score objects match
byte-for-byte; all top pixels outside the mirrored UI match. The bottom
screen's decorative background animation can drift, as with earlier replay
hooks. `kiutrace` supports `mirror 0/1`, `stereo 0..1`, and right-eye output
from `shotscaled` for these checks.

## Bottom-screen UI inventory (from your save states, PLAN phase 1)

Sprite flush callers (return addresses into 0x0092F408), by element:

| Caller | Element | Screen | Notes |
| --- | --- | --- | --- |
| 0x0060DB64 | narration band, 0..320 x 0..56 | bottom | dark strip behind the text |
| 0x0060DB98 | **narration text**, 2 quads | bottom | shadow (colour BF000000, +1,+1), then text (FFFFFFFF), same texture region: one texture holds the whole rendered line |
| 0x009E3068 | **portrait**, one 256x256 quad | bottom | inside 0x009E0000.. which `top_hud` treats as top HUD; only skipped by the height check, tighten it |
| 0x00A192E4 | chapter title text | bottom | not the narration |
| 0x00A1950C, 0x00A196C0, 0x00A19748, 0x00A181D0 | hearts counter and icons | bottom | |
| 0x00A11128-0x00A11CE8 | power panel (was listed as the item box) | bottom | three boxes, the current power in the middle one, 49..87 x 202..240 |
| 0x00A13A74, 0x00A13B68 | power / special weapon icon | bottom | |
| 0x009EA550, 0x009EA800, 0x009EAB24 | "You got treasure!" popup: 9-slice box, the item's icon, the two text lines | bottom | 23..301 x 118..191, slides in from the left, about 200 frames, fades out |
| 0x00A17E70-0x00A181C0, 0x00A0F3xx-0x00A0F6xx | background, frame | bottom | |
| 0x0060D68C, 0x00A0D22C-0x00A0E338, 0x00A04710-0x00A04F00, 0x00A01A4C | pause screen | top label, bottom menu | |
| 0x00A0F9E8 | dialogue illustration sprites | bottom | 0x00A0F734, called at 0x00A18244; omit its black bands/borders on top |
| 0x00A0E9D8, 0x00A0EA60 | boss artwork and baked-in title | bottom | two texture tiles, 0..256 and 256..320, both 0..240 vertically; draw call at 0x00A18204 |
| 0x00A182E4 | speakers during boss/background drawing | bottom | call to the normal portrait renderer, not boss artwork |

Pause screen, from state 5: the bottom battle pass 0x0060D93C calls
0x00A01A1C(ctx+0x243C, ctx, shown) at 0x0060DBB4 when 0x00B52774() says
paused; it draws the dimming (0x00A0D22C, 0..320 x 0..240), Resume
(0..148 x 55..111), Retreat (172..320), Options (35..285 x 124..144), loot
and Remove Display, and Options' pages. The top pass 0x0060D3EC calls
0x00A0E6D8(ctx+0x243C, ctx), the "PAUSED" label, at 0x0060D688 only while
paused. The battle bottom elements (hearts, item box) are still drawn under
it. The pause object's state is at +4 (0 closed).

Pause parts by flush caller (from the user's desktop slot 9, the Options
page, and the first page): first page dimming and band 0x00A0D22C (band
0..320 x 119..149), Options page dimming 0x00A01FC8 and band 0x00A01B40
(0..320 x 47..158; a sub-page's sliders and boxes also come from
0x00A01B40, not across). A row of buttons is frame 0x00A04710, icon
0x00A04B34, words 0x00A04D14 and end caps 0x00A04F00 (three quads), the
Outline's states in the same calls: the first page's Options 26..294 x
119..149, the Options page's four rows from 47 every 27 down. Back arrow
0x00A05C70 (2..49 x 197..238), loot 0x00A0E290, 0x00A0E338 and 0x00A01A4C
(3..74 x 157..193). The Options page's Hint is on the top screen, its bar
0x00A0CA34 (0..400 x 186..240) and words 0x00A0CC04 (shadow then text,
90..310 x 205..221); the first page's Paused label is flushed straight from
the top pass, returning to 0x0060D68C (0..172 x 5..39).

Dialogs (Retreat's "Quit and return to the goddess?") are their own 2D
context (0x14A1340C in state 5). Bottom pass 0x00722A50: 0x00721C64 dimming
and box, 0x00721484 buttons, then the text through the 3D renderer (pointer
at 0x00F2DC9C): 0x00909D54 lights, 0x0090A5B4(renderer, projection 4x4, view
3x4), 0x0090FFA0, 0x00909ACC(text) twice, 0x0090A6A4. The bottom projection
is rows (0, -1/120, 0, 1), (-1/160, 0, 0, 1), (0, 0, 1, 0), (0, 0, 0, 1);
the top pass 0x007255AC uses -1/200 for its own. Top passes draw into both
eyes' buffers: 0x00916588(.., 2) switches to the right eye's (0x1F000000),
(.., 0) back to the left (0x1F080000, the one shown with the slider at 0).
2D sprites go to both through the batch, 3D-renderer text is drawn once in
each. The right eye's target is physical 0x18000000 at 256×512 (dimensions
register 0x011FF100); the bottom screen draws into the same VRAM at 240×320
(0x0113F0F0). The port drops the right eye's draws on the GPU and replays
into the left eye only (see screen-transform.md, One view).

Quad positions are in the element's own space (the narration's text starts at
0,0), placed on screen by a transform elsewhere, not baked into the vertices.

Stacked narration: in your state 1 the line's texture region holds two lines'
glyphs, drawn over each other, so the line texture was not cleared before the
new line was rendered into it. Same with the 60 FPS patch undone afterwards
(the texture was already drawn). Cause to find in the text rendering path
(PLAN phase 2, question 4).

Pickup popup, from your state 8 (6.state of 29 Oct, the Sagittarius Bow):
the popup is its own object at the bottom UI's +0xDC, drawn by
0x009EA06C(popup, ctx), which only reads it (no timers advanced in the
draw). The background drawing 0x00A17F70 calls it at 0x00A18368 in a
normal battle frame, after the portraits, once 0x00B524D8() says so; two
other paths (0x00A18094, and a tail call at 0x00A18030) are for the
bottom UI's other states (+0 bits 0x1000, 0x100000), not seen yet.
The same call draws the item held (used with L, the reticle yellow while
one is): a tab at 286..320 x 206..240 (flush 0x009EAC80), then the icon,
289..321 x 209..241, flushed by a tail call, so the flush returns where
0x009EA06C's call does (0x00A1836C, or the top pass when replayed).

## Powers (miracles, `MRCL` in the game's names)

Found by tapping the power panel in state 1 (Health Recovery, 3 uses) and
tracing what changed.

- Power panel: ctx+0xB64, in the bottom battle 2D context (0x14155330 in
  state 1, the battle UI object 0x00EE3098's +0x140). Words: +0 flags
  (0x800000: no power can be used now), +4 touch state (0x40000000: a power
  asked for, 0x80000000 with it; 0x28000000 busy), +8 command, +0x118 the
  current power's place in the list, +0x184 the list's length, +0x188 the
  list (bytes naming entries). Entry e, 8 bytes at +0x11C + e*8: uses left
  (byte), +1 a byte 0x006DFF2C checks, +2 id (u16, Health Recovery 0x95),
  +5 1 while it can't be used now (Health Recovery at full health).
- Touch: 0x00A129F0 (from the bottom pass at 0x0060DCD8) on a short tap of
  a box (left, middle, right: current - 1, current, current + 1), if it has
  uses and the panel isn't busy, sets +4 |= 0xC0000000, +0x19A = the place,
  +0x19B = the box, +0x1A0 = 0.1, and plays sound 0xB.
- Update: 0x00A17A10(panel), from 0x003EF664. Refreshes +0x121 for the three
  shown, then, if +4 has 0x40000000, the panel isn't locked and the place's
  power can be used, writes the command (keeping `& 0x007E7FEE`):
  `id << 23 | place << 17 | box << 15 | 1` (0x4A808001 for Health Recovery
  in the middle box), clears +4's top two bits and sets +0x19A to 0xFF. If
  it can't, the request stays set.
- Command: 0x00A30D00(UI object, 0 while playing) handles bit 0 with
  0x003636F8(0x00E3F728, player, id) (player able, flags 0x44 clear, uses
  left), then 0x00A17828 for the panel's feedback; the word is cleared after
  (0x00A317A0). Below: 0x0038A3BC(0x00E40978, player, id) ->
  0x003A4CE4 (refuses while running) -> 0x003CC344 -> 0x005A734C, the
  inventory count (0x00E40984 for Health Recovery: 3 -> 2).
- The fork's power key writes that command at 0x00A30D00's entry, when a
  touch of the middle box would ask for it (not locked, not busy, uses
  left, usable now).
- A battle is shown by the top battle pass (0x0060D39C; the fork's hook at
  0x0060D60C inside it): every frame in states 1-5, never on the title
  screen or file select. Paused, it still runs.


## Menu input (pause screen, Options, dialogs)

Found on state 5 with call logging and disassembly (PLAN phase 4c).

- Input record: 0x00A3115C(record, ...) from the input singleton
  (0x002625B8(): +0x2E touching, +0x24/+0x28 the pixel, +8/+0x14 buttons):
  x, y, touching (byte +8), began now (byte +9), key (signed byte +0xA, -1
  none). Keys, from the harness: 2 UP, 3 DOWN, 5 B, 6 SELECT, 8 X, 9 Y,
  10 L and ZR, 11 ZL; 0 and 1 are the other two direction bits (LEFT and
  RIGHT gave none on the main page), 4 button bit 0x100 (A, not seen: B
  had closed the menu). "Began
  now" is set on the touch's first frame only, and that is when an option
  is used (the stylus's press, not its release).
- 0x00A31788 -> 0x0060DC7C(ctx, playing, key, touching, began, x, y): while
  paused (0x00B52774), the key goes to 0x00A0C5A8(pause, key) at
  0x0060DD0C (r5), then the touch to 0x00A01DA0(pause, touching, began, x,
  y) at 0x0060DD24 (r8, r6, r7, y in s16 as an int), the pause screen in
  r9. 0x0060DD28 is after both.
- Pause screen (ctx+0x243C): +4 state (2 main page, 7 Options, 4 closing),
  +0x678 Options' page, +0x6EC the page being gone to, +0x67C the option
  picked, drawn lit; -1 draws none lit (main page, Options' pages, both
  screens), and keys handle it only on pages where the game uses it.
  Main page options: 0 Resume, 1 Retreat, 2 Options, 3 Remove Display.
- 0x00A01DA0 hands a touch to 0x00A0867C (main page) or 0x00A06188
  (Options, when +0x678 == +0x6EC). Both hit test, then at 0x00A08820 (r1)
  and 0x00A06384 (r4) have the option; off every option they return before.
  A different option is picked and plays 0x0075D420(1), the cursor's sound;
  only a touch that began now uses it (on Options' lists, one already
  picked: the first tap picks, the second uses).
- Options' pages: 0-2 lists (0 the back arrow, 1.. the rows), 3-4 choices,
  5 Control Configuration (0x00A0ACF0, drags what it holds), 6
  Reticle/Camera (0x00A0B50C; sliders in the rows above y 70 set by a held
  touch, values at +0x698; starts with -1 picked), 7 Volume (0x00A0C2DC; a
  held touch at x 121..288 of a row sets its slider, values at +0x6AC).
- Dialogs, 0x007226C4(dialog), called from 0x00724738 (Retreat's: 0x14A1340C
  in state 5): only a touch that began now picks a button, which uses it.
  +0x28C how many buttons, +0x2A8 their boxes (4 shorts: left, right, top,
  bottom, inclusive; Retreat's No 36..155, Yes 173..292, 154..188), +0x290
  which cancel, +0x294 the one picked (-1 none; a direction then picks 0).
  The picked one read into r6 at 0x00722754; a change after lights it and,
  at 0x007229E4, plays the cursor's sound unless a touch picked it (r5).

## Menus outside battle (started)

File select (`trace/s1.state`, the three books): RIGHT moves to the next
book and plays sound 0x11 through 0x0036C438, from 0x00BB9C34 (a UI
element's sound) via 0x00AB9810, called through a vtable from 0x0048CB3C,
which runs a list of commands (vtable +0x20, +0x24). So these menus are
scripted, not the battle UI's code, and 0x0075D420 is not used.

Their touch input is one class, vtable 0x00CFC340, on file select (object
0x1403A380 in s1), chapter select and the Fiend's Cauldron (0x14012690 in
s11 and s12). Found by the functions that run only with a touch held, from
`ran` with and without one: 22 in all three states.

- Its touch listener (secondary vtable, slots from 0x00CFC348): press
  0x00A390EC, release 0x00A38D84, held 0x00A38858. Each turns the touch
  into an event and sends it to the object's vtable +0x24: 1 press
  (0x00A38728), 0x40 held (0x00A38650, every frame), 0x10 drag with its
  movement (0x00A38510), 2 release or flick with its speed (0x00A388F8).
  0x00A38858 also draws the blue ring at the touch (0x002FB790).
- Hit test 0x00A38324(out, object, &xy): walks the layers (+0x1EC the
  first, +0x28 child, +0x2C next), each an array of elements (+0x40, 0x110
  bytes each, +0x20 how many), skips those with +0xC0 & 6 or in another
  group (+0xC8, the object's +0x644), tests the point against the
  element's triangles (0x0025F278, +0x9C) and keeps the frontmost (+0x80).
  Out: layer, index (-1 none), the element in r3 on return. On s1: book I
  0x1406D150, II 0x1406D9D0, III 0x1406E2D8, Back 0x1406E938, How to Play
  0x1406EE00, nothing at 300,20 or on ?HOME.
- The screens act on the press only. With press and release returned at
  once (`code 0xa390ec 0xE12FFF1E`, the same at 0xa38d84), a held touch
  on book II, Chapter 2 or Back does nothing. A press on a book or chapter
  lights it (the second press opens it); a press on Back uses it. So a
  held touch cannot light anything here, unlike the pause screen.
- 0x0071C420 reads the touch each frame (the input singleton, +0x2C..)
  and calls the listener: +0xC press, +0x18 held still, +0x10 moved, +0x14
  release (a tap if under 40 px), +0x1C no touch (0x00A391E4 here, empty).
- A press needs no touch: 0x00A38728(object, &xy) called where 0x00A391E4
  runs lights book II, and a second opens it, with no ring; on chapter
  select it lights Chapter 2 without the list scrolling (a real tap
  scrolls it, on its release). A press on what is already lit uses it:
  one on book I, lit as file select opens, opens the file.
- The event's vtable +0x24 is empty (0x00B6F668); 0x0036C198 posts it as
  a message (0x002F3464's queue), and each screen's own class handles it.
  File select's (0x003025B8, run only on file select) compares the
  element's id with its parts (+0x9C, +0xC8, +0xF4 of its table the
  books) and the picked one, +0x34: the same opens it (0x003374D0),
  another is picked and its book turns (RotObj0000). The lit book's index
  is at 0x1404E9EC in s1 (0, 1, 2). Chapter select's (0x0031164C) only
  stores the flags and position (+0x5B8.., file select's at +0x508) for
  its update and 0x00344400. What a press does, and what is lit, is each
  screen's.
- The element's +0 points at its part's first name (Book01, ML_B0002,
  BL_B0000 for Back, Move Out, the pot and the bar alike) and +4 is 2 or 3
  on chapters: neither tells a picking part from a button.
- Parts are named and known by a hash of the name. The hit test's layer
  +0x38 is the part's object; 0x00326D74(it, &0x00275C18()) +0x1C is its
  descriptor (vtable 0x00CFEB2C): +0x18 the hash, +0x1C the name (a
  string object). Book II's layer leads to 0xA70D5C64 "book_02".
- File select (object 0x1404E918 in s1, vtable 0x00CEC3F4): the part with
  the outline at +0x34 (hash) and +0x38 (name), the book at +0xD4. Its
  update (0x002BBDE8) finds the part's entry in its navigation table
  (0x003023F0: +0xC the part, +0x38 right, +0x64 left, +0x90/+0xBC
  return, +0x140 guide, each a hash and a name; book_01, book_02,
  book_03, return, guide) and moves the outline with 0x00302488(screen,
  &{hash, name}), or LEFT/RIGHT through 0x003381AC(screen, -1/1); A is
  0x003374D0(screen). Called where 0x00A391E4 runs (the harness copy's
  `callf`), 0x00302488 with book_02 outlines book II, the panel follows
  and sound 0x11 plays (0x00BB9C34, 0x0036C438) as for RIGHT, and nothing
  opens; 0x003374D0 then opens it. A descriptor's +0x18 is the {hash,
  name} 0x00302488 takes.
- File Select replay/editor (3 Nov): the 2D foreground-material override
  removes the 3D book covers while leaving their numerals and names. In
  `s1`, the red Book II sample has zero artwork pixels with that override,
  versus 322 after retaining native materials/depth; the native bottom
  sample remains 1,918 pixels. `MenSelfileBt0000/1/2` are Book I/II/III,
  `MenSelfileBt0003` is Back, `MenSelfileBt0004` is How to Play.
  Their hashes are `A70D5C65/A70D5C64/A70D5C67/928BEAA4/8D79D12E`.
  `MenDemoBt0000` is the bottom background; HOME text is a separate font
  draw, not an activatable Control. `MenSelFileUp0001` in layer B contains
  both the information Board and the entire upper backdrop. Its `Bg0007`,
  `FileNum*`, `MiiDynamic0000` and `ClrMarkN` nodes belong to the Board.
  `NN_U0000/1/2` are paper/lines/shading; `NN_U0004/0010` are coloured
  ribbons. `NN_U0005`, `NN_U0003`, `NN_U0006`, `NN_U0007_1` are top-left,
  bottom-left, bottom-right and right-edge decorations respectively;
  `NN_U0007` is the feather and `NN_U0009` the coloured crest.
- The File Select editor indices are 0..2 books, 3 Back, 4 How to Play,
  5 HOME hint, 6 information Board, 7 bottom background; 8..15 expose the
  upper backdrop's mesh groups independently. Its explicit replay order
  must include those appended layers before the Board and Controls.
  Native book-name font
  anchors remain in their respective thirds of the 320px bottom screen,
  above y190; HOME anchors are around (102,207) and (68,226). Select a
  book's name by that native column, preserving all native shadow draws,
  and let its 3D cover turn outside the nominal Placement source box.
  The native hit test validates those extensions, not a second hit layout.
  Retain the original full-Board viewport/projection and expand only the
  book's scissor. A full-canvas isolated projection displaces the native
  font draws and reveals book faces normally clipped off-screen. The
  corrected capture has zero unexpected pixels to the left of the placed
  Boards, with names attached to their books at normal and wide aspects.
- File Select's black cross-screen connectors are the `RotObj*` mesh
  nodes: `RotObj0000` on Book I and the information Board, `RotObj0004`
  on Book II and `RotObj0005` on Book III. Hide those nodes only in
  transformed File Select replay; retain `Book01/02/03` and `Book04/05`
  cover/page meshes, native materials, and the original bottom pass.
  Suppressing `Book04/05` did not remove the connectors and was reverted.
  The capture strip above the covers changed from 76/51/34 black pixels
  for selected I/II/III to 0/0/0 after suppressing `RotObj*`.
- Editor visibility is stored separately from geometry as hidden
  `(screen, part)` pairs. Hidden entries retain their editor indices and
  source boxes but leave draw order, pointer candidates and explicit
  navigation destination lists. An empty draw order for an active Screen
  must not fall back to its unfiltered parts. File Select HOME is index 5;
  a moved HOME rectangle changes from 4,800 visible pixels to zero when
  hidden, survives a console-state reload, and returns when shown.
- Cardinal File Select seams, after native repeat checks and accessor
  `003023F0`: Left `002BC5A4`, Right `002BC618`, Up `002BC688`, Down
  `002BC6CC`. r0 is the current graph node, r5 the Screen object. A linked
  action calls native `00302488(r5, target_node+0xC)`, which sets both the
  selected hash/name and book index before calling `003381AC(...,0)`.
  Resume at `002BC66C` after Left/Right or `002BC6E8` after Up/Down so
  the remaining native input handling stays intact. A self-link resumes
  without calling the Outline helper. A plain directional-slot bias is
  insufficient: native Left/Right also performs its adjacent book step.
  Live checks cover all four directions to Back, I→III and III→I with
  matching `+0xD4` indices, a held self-link, native-default restoration,
  circle-pad input, and native A opening How to Play.
- Chapter select (object 0x1403E7A0 in s11, vtable 0x00CF243C): the part
  with the outline at +0x6C/+0x70 (BtThumbnail on the chapters; DOWN
  moves it to Return through 0x003668E8, which posts it). The chapter is
  +0x12C (and +0x130), the last one open +0x128. 0x00343FF8(screen,
  chapter, 1) picks one as LEFT/RIGHT do (0x003441F8): called with 1 it
  outlines Chapter 2 and scrolls the list, but without RIGHT's sound 7,
  which comes another way.
- So the outline is each screen's own: a hash and name at its own offset,
  and its own function to move it and to use what has it. The part under
  the mouse is the same for all of them.
- The hit test's out: +0 the layer, +4 the element's index (-1 none), +0xC
  the element. The task loop runs a menu's update through a thunk, the
  object in r0: file select's 0x002A3EC4 (to 0x002BBDE8), chapter
  select's 0x002A3FCC (to 0x002D7404, then 0x00310880 for its keys).
- Chapter select's list is one layer (0x140511D8 in s11) of 33 elements:
  MenStageselectBt, ML_B0000, ML_B0001 to ML_B0026 (each chapter, named
  by its number; ML_B0009 and ML_B0023 also _0 and _1), NN_B0000. The
  chapters not open have +0xC0 0x2002 and are not found. The index stays
  with the chapter as the list scrolls (Chapter 1 is 2, Chapter 2 is 3).
  The element's +0 is its name here, so the chapter is the number less 1.
- Chapter select: 0x00367394(object) outlines the list (BtThumbnail) as
  UP does from Back, 0x003668E8(object) Back (Return), each posting it;
  0x00343FF8(object, chapter, 1) scrolls to the chapter and sound 7 plays
  as for RIGHT. 0x003675C0(object) is A: on the list, the chapter's
  Fiend's Cauldron (0x0038B214), on Back, back (0x003672BC); from s11 it
  goes on to the cauldron the same frames as A.

- Main menu (`trace/s14.state`, your 1.state of 30 Oct): object
  0x1403D640, vtable 0x00CFBD9C, updated through 0x002A3F9C (to
  0x002EBA60, which redraws the outline when +0x9C is set, then
  0x0034E28C). Its keys, 0x003201D8(object, input): directions step a
  table by 0x0034E1B8(object, direction; 5 RIGHT), which writes the
  outlined part's hash at +0x44 and its name (a string) at +0x48, copies
  them to +0x70/+0x74, calls 0x0036FB58(object, 0) (reads only +0x44 and
  +0x48, sets +0x9C) and plays the cursor's sound, 0x002B435C(+0x30, 7).
  A (input 0xF500) is 0x0036F59C(object); B 0x200 goes back. The hit
  test's parts carry the same hashes: Solo 2E49363F, StreetPass 5407F273
  ("Present"), Vault E250C577, Together E2DCC735, Options D929D345, Back
  9A07E025, AR 4A7AE3E3, guide FD90A21D. Functions of the 0x0034E/0x0036F
  family may be shared by the hub's other button menus.
  The shared directional-name resolver is `0x003506C4(record, direction)`.
  Cardinal codes observed with real pad input are Up 7, Down 1, Left 3,
  Right 5. A graph record contains its own hash/name at +0xC; linked records
  are reached through +4. Direction 9 takes the default/self-name path.
  Editor links redirect this resolver to the destination record with direction
  9, leaving native repeat, availability checks, name copying, sound and A
  activation in the original caller. The s14 replay confirms four cardinal
  overrides, circle-pad Right, a held self-link, and native fallback after
  removing a link. Diagonal codes are not overridden.
  Solo Hub inlines the directional record reads after `0x0034E870` returns:
  `0x00321720` Left (+0x64), `0x0032178C` Right (+0x38),
  `0x003217F8` Up (+0x90), `0x00321864` Down (+0xBC).
  These hooks bias the returned record so the caller reads the destination's
  self-name through its existing slot. Save-state checks redirect Move Out
  Right to Practice Range and Arms Altar Redeem Hearts Right to Back.

- Solo hub (`trace/solo.state`, from s14's Solo): object 0x1403A5E0,
  vtable 0x00CFC3D0. Its live update is 0x002ECCF4 (called directly by
  the task dispatcher, rather than through thunk 0x002A3EF8). The current
  hash/name is +0x5C/+0x60, pending +0xFC/+0x100. Directions in
  0x00320E7C call 0x002BFE50(object+0x4C, part), queue the pending name,
  and set +0x150 bit 0. The update posts its notification, copies pending
  to current, and moves the camera by 0x00321D28. A is 0x00370300(object),
  also used by the touch handler 0x0034E944. +0x144 == 1 is the opening
  guide, which owns input. Hit-test hashes: Move Out 79BCE02D, Arms Altar
  38C8C232, Practice Range 94CE5E14, Idol Toss 4F73A4C0, Gear 71966FCC,
  Treasure Hunt ED263933, Back 9A07E025.
  A click that needs to change the outline must let the update consume
  its queue before calling A: otherwise clicking Arms Altar after DOWN
  moved to Gear opens Gear. Shared menu call handling now waits for the
  next update after its final outlining call before using the choice.

- Arms Altar's button menu (`trace/altar.state`, from Solo's Arms Altar):
  object 0x1403E7C8, vtable 0x00CFCA28, input 0x002A1FD8 (called by
  active update 0x0025F940 and transition update 0x0025FAEC). Current
  hash/name +0x44/+0x48. Directions go through 0x002EF084 and touches
  through 0x002EF154; both call 0x00322DB0(object, &name, repeat), which
  moves the outline, posts its notifications, updates the animations and
  plays the cursor sound. A is 0x00322C34, Back 0x0032287C. Hit-test
  hashes: Redeem Hearts 1B80430F, Fuse Weapons C59BD435, Convert to Hearts
  E43D88E9, Gear 71966FCC, Back 9A07E025. The mouse calls 0x00322DB0 with
  repeat 0, then A after a click. RIGHT from Redeem Hearts selects Fuse
  Weapons; clicking with the pointer still over Redeem Hearts correctly
  returns there and opens the shop. Harness: `trace/menualtar.txt` and
  `trace/menualtar-{fuse,convert,gear,back}.txt`.

- Mouse gestures: the frontend no longer consumes a mapped menu button
  on mouse-down. `AimMouse::menu_touch` waits for release to call A, or
  passes a held native touch after 12 emulated frames or four pixels of
  movement. It begins at the original point for three frames, then
  follows the pointer, so fast movements still grab the original item
  through a 30 FPS input poll. Unmapped controls pass native touch
  immediately. `outline_menu` leaves native touch and its release alone;
  losing focus cancels the pending click. The frontend polls held input
  even without cursor events. Harness `press <frames>` / `release` uses
  the same routing; power-board pickup, drag and release match native
  touch on both screens (`trace/powerdrag-{mouse,native}.txt`).

Armory mouse support uses seven native input updates: Gear 0x002B6DB8,
Redeem 0x00238CBC, Convert 0x001C9734, Fuse 0x00230B5C, Powers
0x002BB14C, weapon list 0x0019CDC8, categories 0x0019BDC4. Their A/B
event records preserve native validation and confirmations. Generic lists
keep absolute selection at +0xE4, first visible row at +0xEC, row height
at +0xFC and animated offset at +0x11C; 0x0031AA88 scrolls their viewport
and 0x0031AED8 refreshes the cursor. Fusion uses independent horizontal
and vertical offsets +0x2B28/+0x2B30, bounded by count +0x2B40 minus
seven visible items. Category navigation uses native direction tables and
skips unavailable categories. Part-component tree lookup terminates at
its head sentinel; +0x10 is node color, not a leaf marker.

- Options and its submenus (states s15 Options, s16 Other, s17
  Controls, s18 Volume, s19 Profile, s20 Subtitles, s21 Reticle/Camera,
  s22 Control Configuration, s23 More Controls, s24 Music Gallery, s25
  Profile's Yes/No, s26 Air Battle's settings, s27 SpotPass's No/Yes; all
  made headless from s14). Each is its own class, most a copy of one
  pattern: the outlined part at +0x44 (hash) and +0x48 (name string); a
  key queues the next one and an early step of the update (a copy of
  0x002B4428) outlines it, posts it to the buttons' animations and plays
  the sound from a scripted element (0x00BB9C34). Per screen (vtable,
  update thunk, how a key queues, A):
  - Options 0x00CEAE98, 0x00224BA0: 0x002BFE50(+0x34, part), then
    0x002EA7A0(+0x9C, part), +0xD4 |= 1 (bit 1 back); A 0x002FB9E0.
  - Reticle/Camera 0x00CF9B60, 0x001B4C74: as Options; A 0x0029B26C.
  - Other 0x00CF2368, 0x001ECB4C: one function, 0x002D5430(object, part,
    direction), which also writes +0xC8/+0xCC 2 for SpotPass. Left 0
    (queueing by hand), SpotPass's switch (0x001CFF5C) takes the move for
    a touch on it and sets the touch object's group, +0x654, to its own
    (0x12): the hit test then skips every other layer, and no touch (real
    or the mouse's) finds Profile. A 0x002D50FC.
  - Controls and More Controls 0x00CF84DC, 0x00224C18: 0x002EA7A0(+0x9C),
    +0x118 = 1, +0x108 = -1, +0x11D bit 3 direction, then
    0x00318DD0(object, part) for the rows' animations; outlined by
    0x002E1CF8. A 0x003186E8 (toggles a More Controls row).
  - Volume 0x00CF2390, 0x00224BD4: 0x002D5B40 queues at +0x9C, zeroes
    +0xE4/+0xE8, +0xD4 |= 1; outlined by 0x002D562C. A 0x002D56CC.
  - Profile 0x00CF8E8C, 0x001ECBF4: hash +0xA4, name +0xA8, +0xDC bit 0;
    outlined by 0x002952F4. A 0x002E2F4C.
  - Profile's Yes/No 0x00CEC41C, 0x0016756C: hash +0x9C, name +0xA0, +0xF0
    bit 0; outlined in 0x00172A50. A 0x001C16D4.
  - Subtitles 0x00CFA034, 0x001ECBBC: the outlined one an index, byte
    +0x60, into parts at +0x18 (0x2C each) of what +0x30 keeps of kind
    0x00D70E54 (the map 0x00326D74 walks: a sorted tree at +0x1A8/+0x1AC);
    0x002E5608(object, part, direction) outlines one. A 0x002E5448.
  - The hub's choice box (SpotPass's No/Yes) 0x00CF8270, 0x002A4070: one
    part (BtDialog), each button an element fnt_02, fnt_03 (index 0, 1 at
    +0x44); keys set the index then 0x0028F8EC and 0x002DC68C light it;
    0x00314DF8(object) uses it (A and a touch).
  - Music Gallery: a scrolled list, its cursor moved by 0x002576C4(object,
    step) (0x0021E7C0 its keys); a tap picks a row. Control Configuration
    and Air/Land Battle's settings: no outline posted on keys; touch
    screens (sliders, a test pad).
- Two of these update at once while a choice box is over its menu (Other
  under SpotPass's box), so the mouse goes to the newer of the two.
- A save state made on Windows keeps each open archive's Windows path;
  loaded here the game could not open gamecoin.dat (shared extdata) when
  Options opened and stopped (0xC8804471). Loading now moves each archive
  under this run's user directory.

- Practice Range (`trace/practice.state`, from Solo's Practice Range):
  object 0x1403E7C8, vtable 0x00CF0154, its update 0x0023A9FC (through its
  thunks 0x00224C50, 0x00224C54, 0x00224C58). The outlined part a hash at
  +0x2E4 with its name after it (+0x2E8, a string whose buffer is at +0x20).
  A key writes the next one's name into a queue at +0x33C and sets +0x374's
  bit 0; the update's step 0x00283DE0(object) copies it in with
  0x002EA7A0(+0x2E4, +0x33C), which computes the hash, plays the cursor's
  sound and marks the part. A is 0x0030B5B4(object): Air Battle and Land
  Battle start their practice with 0x00340FF4 and 0x00340ED4(object, 0 or
  1), Gear opens the armory with 0x0030B68C(object), Back (the return hash)
  goes back. The parts: AirTraining 0xE50FA312, GroundTraining 0xADD1FD65,
  Equipment 0x71966FCC, Return 0x9A07E025. On the bottom screen Air Battle
  is about (160, 50), Land Battle (160, 150), Gear (160, 215), Back (20,
  200 to 230); the left arrow and the top screens' titles and hints are the
  game's own for the key.
- Idol Toss (`trace/idols.state`): object 0x14040508, vtable 0x00CF9400,
  its update 0x001B4D58, a thunk of 0x001E05D4, which reads the pad into
  +0x2E0 and +0x2E4 and calls 0x00217A0C(object). The selected part an
  index at +0x3B8 into the scene's parts at +0x44, +0x254 and +0x1D0 (the
  scene is what its task, 0x1403D258, keeps of kind 0x00D71458), a hash at
  +0x48 and its name after it. Its keys step the index with
  0x0024E9F8(object, input) and copy both; a touch, 0x001E0B60(object),
  calls 0x0022CF38(object + 0x38, +0x48), which finds the part under it.
  The bowl's hash is 0x4F14908C, Back is 0x9A07E025. These parts act on
  touches and not on A: the bowl does nothing at this save's point, Back
  goes back on release.
- Treasure Hunt (`trace/treasure.state`): object 0x14040508 as well,
  vtable 0x00CEEDD8, its update 0x0018AF44, a thunk of 0x001A0C64, the
  pad's bits into its elements' flags; 0x001A0AE4 its other update. What
  is picked a hash at +0x34 (Back 0x3B8915AB) and its name after it;
  0x0020207C(object, part) sets both from a part, as its touch handler
  0x001CBFCC does. The board's tiles are not parts the hit test finds, so
  the mouse outlines none of them: its press touches where it is, and the
  tile under it is picked, its idol shown on the top screen.
- Vault (`trace/vault.state`, from the main menu's Vault): object
  0x1403A5E0, vtable 0x00CF8248, its update 0x00224B80, a thunk of
  0x00240164, which copies the pad into +0x2D8..+0x2E8 and then runs
  0x0028E6AC (the input's bits into the parts' flags at +0x194, +0x1B8
  and on, 0x24 apart) and 0x0028E5E4 (the hash at +0x44 to a part index,
  0..7, and its scene's data to +0x70's +0xC). The outlined part its hash
  at +0x44 and its name after it; 0x00345C18(object, part, 1) sets both
  from a part, as the class's pointer handler does (0 for a key), and A is
  0x00345FDC(object), which presses whatever hash +0x44 holds. The parts:
  Idols E1D05CA4, Music Gallery 0A9FA648, Power Portrait ABC93BCF,
  Offering 04E786EA, Idol Toss 84D28812, Records 9EC09AD0, AR BEA7E6CA,
  Back 9A07E025; on the bottom screen Idols is about (100, 60), Music
  Gallery (250, 60), Power Portrait (60, 145), Offering (160, 145), Idol
  Toss (240, 145), Records (160, 195), AR (285, 225), Back (40, 225).
  Vault's direction handler calls the shared resolver `0x003506C4` (return
  address `0x002DC4F4` in `f_002DC458`) with the current part's hash at
  record `+0xC`, so editor links redirect it exactly as the hubs do. Verified
  from `trace/vault.state`: Right from Idols with a link reaches Back
  (9A07E025), and removing the link restores Records (9EC09AD0).
- Options and its Scene child pages: Options' four parts are Options
  6C7DCDF1, Volume 53977FC2, Other FB57B10C and Back `928BEAA4` — Back is
  *not* the shared 9A07E025. Practice Range's are Air Battle E50FA312,
  Land Battle ADD1FD65, Gear 71966FCC and Back 9A07E025. Their key handlers
  inline the graph lookup rather than calling `0x003506C4`. Options gets
  its current node from `0x002B481C`, returning to Left `0x0026E0A0`,
  Right `0x0026E114`, Up `0x0026E18C`, Down `0x0026E204`. Practice gets
  it from `0x002CF954`, returning to `0x00284898`, `0x00284948`,
  `0x002849F8`, `0x00284AA8` respectively. Their name slots are +0x64,
  +0x38, +0x90, +0xBC, as Solo's. All eight cardinal redirects to Back
  passed native Cursor assertions. The earlier `0x003D4114` observation
  concerned graph rebuilding, not directional input; compiled callees
  were invisible to the old `calls` probe. `calls` now interprets its
  requested targets before stepping.
- The Options child Screens use the same circular navigation nodes
  (current hash +0xC, next +4) despite different current-node accessors.
  The editor hooks these accessor returns before the caller reads a name:

  | Screen | Left | Right | Up | Down |
  | --- | --- | --- | --- | --- |
  | Other | 0023C1AC | 0023C238 | 0023C2C8 | 0023C384 |
  | Controls/More Controls | 002E135C | 002E1854 | 00293848 | 002938CC |
  | Reticle/Camera | 00219A64 | 00219AD8 | 00219B50 | 00219BC8 |
  | Profile | 002474B0 | 0024751C | 00247588 | 002475F4 |
  | Profile Yes/No | 0019560C | 0019568C | 0019570C | 00195784 |
  | Subtitles | 002535C0 | 00253610 | 00253660 | 002536B0 |

  Their current-node accessors are 00289880, 00319228, 00252168,
  002958E8, 001C1DC0 and 0029B8BC respectively. All cardinal redirects
  passed native Cursor checks, including Subtitles' byte +0x60 (Off = 0).
- Armory — Gear (vtable 00CEC2B8) uses the same graph through its accessor
  `0x002FCD34`, from key handler `f_002B6DB8`, outlining with
  `0x00334D3C(object, name)` and keeping the hash at +0x2E4. The editor
  hooks its returns: Left `002B75E4` (+0x64), Right `002B7638` (+0x38), Up
  `002B7688` (+0x90), Down `002B76E0` (+0xBC). Parts: `MenEquipBt0001`
  Weapons/A754BFE8, 0002 Powers/CE77C7CF, 9999 Back/9A07E025, 0000 the
  backdrop. Verified from `trace/armory-gear.state`: Down reaches Powers
  natively, Back when linked, and Back's linked Right reaches Powers.
- Armory — Categories (00CEE1C0) steps an index, not names: tile at +0xDC
  (0 All to 9; 10 Back), previous at +0xE0. Its key handler (from `0x0019C470`) calls
  `0x001C66BC(object, table)` per direction, Left 00CA3180, Right 00CA2AA0,
  Up 00CA2730, Down 00CA2E10 (four more are diagonals); each table has 11
  rows of 0x28 bytes listing candidates, -1 terminated, the first with
  weapons (`0x001FC6C4(tile - 1) > 0`) taken at `0x001C670C` from r4. From
  Back the step returns to +0xE0 whatever the candidate. The editor hooks
  the loop head `0x001C66CC` (r6 = 0) to take a linked tile, writing +0xE0
  first when leaving Back; `0x001C66E4` returns 0 for a self-link. Tiles
  are `MenEquipBt0102`..`0111` in index order, Back `MenEquipBt9999`.
  Verified from `trace/armory-weapons.state`: All→Back, Back→Bow and Bow's
  self-link, with native Down from Bow reaching Club (6).
- Armory model roles: Weapons list `MenEquipBt0120` rows (0125 cursor),
  0121 order (X), 0122 page bar, 0123 Equipped (Y), 0100/0101 backdrop.
  Powers `MenEquipBt0203` grid and tabs (0206 pieces), 0211 list (0215
  cursor), 0204 Autofill, 0212 category header, 0205 OK, 0200 backdrop.
  Replaying the whole Board for each Control stops the top replay after a
  few frames (drawing buffer); each Control draws its own models, and only
  Controls that show text replay the text pass.
- Armory text replays must filter glyphs by the Control's native source
  rectangle in `menu_top::font`, before the game's glyph allocation. A
  scissor alone still allocates all of the dense bottom text pass for each
  text Control, exhausting the shared drawing buffer on Powers entry with
  larger inventories. This froze the game mid-transition (oversized grid,
  truncated words, Back unresponsive). Reproduced from the user's Chapter
  Select slot 3 through Solo Hub and Gear; omitting either the grid replay
  or the repeated text passes avoided it. Filtering text ownership allows
  the complete entry and Back to Gear without hiding any Control.
  Verified three entry/Back cycles on Windows at 6x using that save and
  the user's current game DLL; all 238 core tests pass.
- Idols node ownership (`trace/idols-viewer.state`, one idol's description;
  its List is Back from there, MENUID 46 for both, Screen 26). Top:
  `MenColViewerUp0000` backdrop, `maru_kage` shadow, the idol a `Dol*` model
  of its own name (`DolSnipe04` here). Bottom layer F: `MenColViewerBt0000`
  nodes `bg` background, `sousa`/`sousa_base` controls hint, `title`,
  `L_button`, `R_button` title bar, `bar` zoom bar, `pbase` page pill (the
  description page's), `base` the white panel and `NN_B0002` its words'
  node; `MN_B0004` stays hidden. The List adds `MenColViewlistBt0000`
  (frame, its own page pill), eight `MenColViewlistBt0003` Rows,
  `MenColViewlistBt0004` the Cursor and `MenCmnScbar` the scroll bar. Layer
  0x10 holds `MenColViewlistBt0005` (the Rows' mask), layer 0x11
  `MenColViewerBt9999` (Back) and `MenColViewerBt0002` (AR: `base`,
  `cursor`). The model log walked layers 0..16 only and missed 0x10..0x12,
  which `BOTTOM_LAYERS` (0xF..0x13) covers. Bottom text anchors: Row names
  x 89 and counts x 262 every 25 from y 15; the List's "1/9" (160, 220); the
  title (150, 38), the description block (14, 64), its "1/9" (151, 211).
- The Idols floor shadow is not `maru_kage` and not part of the backdrop
  (`MenColViewerUp0000`: one mesh, one texture `MenColVwUp_Dynamic00NN`,
  the category emblem; the gradient is vertex colour). It is a
  `CircleShadow` (manager built by 0x00238B5C, 12 slots): a one-vertex
  geometry-shader sprite (stride 64) sampling the 16x16 radial texture
  hashed `519EDA1CAB804F1C` (Citra rows hash). It draws in every split-layer
  replay at that replay's Placement, so a moved Backdrop carries a copy.
  Mesh visibility (+0x24), model +0x1F6, `maru_kage`'s matrices and its
  material depth test all leave it drawn; `maru_kage`'s mesh has no node
  (+0x26 = -1). A texture pack blanking that hash removes every copy.
  The draw is 0x0080B100(this): count at +0x14 (returns at once when 0),
  positions (12 bytes each) at `*this`, offset by the camera at
  `r1[1]+0x28 -> +0x134`, written to 64-byte vertices at this+0x10; the
  texture bind goes through 0x00152714 (called at 0x0080B22C). Found by
  watching the command-list words that bind the shadow texture (GPU reg
  0x85 = 0x18142D80 >> 3) with `KIUTRACE_WATCH_REGS=1`. The Shadow layer
  owns `CIRCLE_SHADOW`; `menu_top::circle_shadow` returns from that
  function in every other replay, so the Backdrop stretches without a copy.
  Back and AR are no glyphs. A saved List state reloads with a black top;
  reach the List from the description state instead.
- Idols Emblem (`MenColViewerUp0000`'s only material, `Dynamic_00`, its
  texture a by-name reference that code binds per category from
  `MenColVwUp_Dynamic00%02d`, string at ROM 0x1534D0; the 20 emblems are
  ETC1 64x64 in `us/menu/460.zrc`, white for no emblem). The runtime
  material (vtable 0x00D1510C) points at the file's `MTOB` at +8, no copy
  at +0x14. Texture coordinator 0 at `MTOB`+0x16C: Scale U/V +0x10/+0x14,
  Rotate +0x18, Translate U/V +0x1C/+0x20, flags +0x24 (0x100; the engine
  rebuilds the matrix when bit 0 is set and clears it; writing the matrix
  at +0x28 alone does nothing). Texture unit 0's commands at `MTOB`+0x2F0:
  border colour, header 0x804F0081 (regs 0x81..0x84), size, sampler
  parameters 0x26 (ClampToEdge); writing them takes effect. Measured with
  a black border and the Backdrop at 0 0 400 240: identity fills
  (248, 25, 114x113); Scale divides that box about its lower left corner;
  Translate moves it right and up by its own size, independent of Scale.
  Outside that quad the mesh's UVs vary on one axis only, so a texture
  moved past it smears; ClampToBorder with a white border keeps a shrunk
  emblem clean.
- Idols view state (viewer object 0x14040508 in `trace/idols-viewer.state`,
  ticking at 30 Hz): zoom `+0x168` (0 out, 0.5 default, 1 in; X/Y and the
  zoom bar), tilt `+0x170` (game clamps to ±90, limits at +0x1C4/+0x1C8),
  angle `+0x174` (degrees, wraps at 360), position `+0x178`/`+0x17C` with
  twins `+0x184`/`+0x188` (re-clamped every tick to a zoom/tilt bound),
  camera offset `+0x194` from zoom. Writing angle, tilt and zoom sticks.
  Idle spin byte `+0x524` (the bytes after it are other state; 6 spinning, 0 stopped; first input clears it at
  0x0024CE78). Stylus momentum is a velocity at `+0x2D4`/`+0x2D8` that only
  the game's own release path sets; poking it does nothing, so the PC Orbit
  keeps its own. Writers: 0x00297E80 (zoom 0x00298030, tilt 0x00298264,
  angle 0x00298280), position in 0x0024C270. L/R and B restore the idol's
  own default view and spin. On the List page the view takes no input.
  Position x is right, y up; the picture is drawn from the twins (`+0x184`
  x, `+0x188` = y - 9), so a write must move both. Default zoom limits are
  x ±19.6, y 12 up to about 24. A position unit spans about
  1 / (0.147 - 0.098 * zoom) native top pixels (6.8 at zoom 0, 10.2 at
  0.5, about 20 at 1; measured from the idol's centroid). The game Orbits
  with the C-stick (the PC right stick) by itself: about 5°/frame at full
  tilt after a short ramp, stick right raising the angle, stick up lowering
  the tilt, nothing below about 0.2 of tilt.
  The PC's Idol View Orbit binding (`bindings::Action::IdolViewOrbit`,
  default the right stick, a `Source::Stick`) replaces that while the Idol
  View is shown (`menu_top::idol_view_shown`): the frontend zeroes the bound
  stick's native axes and feeds its lean, past the aim dead zone, to
  `idol_view::STICK_ORBIT` (6° a 30 Hz tick, 180°/s), with Land Battle's
  inversion.
  The Idols List is the generic list class at screen `+0x330`; its Cursor
  is screen `+0x40C` (list `+0xDC`). Its keys call 0x0031AA88(list, down,
  1, 1) from 0x00297E98 once per Row, stopping at the last Row. The wheel
  makes the same call per notch, on the List page only.
  The idol shown is screen `+0x414` (List Row's idol; L/R and the Cursor
  change it). On the frame it changes the game resets zoom, tilt and
  position but keeps the angle, which the idle spin then turns; the drawn
  y twin settles over the next frames.
- Reticle/Camera choices live in the save singleton `*0x00D636C4`
  (0x00EE33C0): flags byte `+0x163EC` (Land Invert Horizontal 0x02, Invert
  Vertical 0x04; Air Invert Horizontal 0x08, Invert Vertical 0x10, Invert
  Vertical Movement 0x20), Land speeds `+0x163D0` (horizontal) and
  `+0x163D4` (vertical), Land stop `+0x163C8`, Air speed `+0x163D8` and stop
  `+0x163CC`, each -1..1. 0x00B12218 applies the inversion bits after the
  aim delta each frame; written as soon as the Row is touched.
- Idol Toss bottom-screen node ownership (your slot 4 of 16 Nov, harness
  `models`, each node drawn alone). `MenColVisionBt0005` holds the
  counters and the scenery: `BL_B0000` the Play Coin box, `coin`,
  `NN_B00010`, `NN_B0006` the NEW bar (Counters); `NN_B0007` the gold
  pedestal (Fountain); `NN_B0005` the egg rail with its black right edge
  (Egg queue); `NN_B0002` the curtain, `NN_B0000` a hanging piece and
  `NN_B0004` the black ground, left off. `MenColVisionBt0006`: `MN_B0000`
  the water (Fountain), `MN_B0001` the mist, a band 20..304 across that
  ends at the screen's bottom, left off with the ground it lay on.
  `MenColVisionBt0002`, five of it: the queued eggs. `Bt0000` the bowl,
  `Bt0001` its egg and arrow, `Bt0007` the NEW tag. All of them draw in
  the opaque pass; the translucent pass (0x007C90F8) adds the bowl's
  orange shading, and is now drawn with the Launcher, the Fountain and
  the Egg queue only. The rail and the water end at the bottom screen's
  edges in the meshes themselves, not at a viewport. Node flags poked on
  the native bottom screen are rewritten each frame; hide nodes inside a
  replay instead. Windows release rebuilt as `dist/zakuro-kiu-mouse.exe`,
  SHA-256
  `bcd4ec2605adfb9eb3a68833c66a2c6670363710d02280c14635b06e3b071f98`; the
  previous build is `dist/zakuro-kiu-mouse-before-toss-scenery.exe`.
- Gear top-screen node ownership: `MenEquipUp0000` nodes `NN_U0000` and
  `NN_U0001` are Paper background, `NN_U0027` Silhouette, `NN_U0002` Crest,
  and its remaining nodes Ornaments. `MenEquipUp0001` nodes `NN_U0000`
  and `NN_U0001` are Page title, `NN_U0002` and `NN_U00022` Weapon.
  Other nodes stay in the native Details card, including the Powers grid.
  The layout row declares these groups; replays retain native node visibility
  and restore it afterward. The doubled camera guard band enlarged this
  artwork despite its Placement; these groups use the native viewport mapping.
- Armory — Fuse Weapons top-screen node ownership (Scripted Screen 17,
  MENUID 26; the user's desktop slot 4 of 20 Nov, a 0.2.32 state).
  `MenAltermixUp0000`: `NN_U0000` Paper background (with both weapons'
  coloured headers, one mesh), `NN_U01` Ornament, `NN_U0003` Weapon A,
  `NN_U0005` Weapon B, `NN_U0009` the yellow filled bonus slots of both
  weapons (one mesh, showing where a dark bar is hidden), `MN_U0001..0006`
  and `MN_U0007..0012` the dark bonus slots of A and B (`MN_U0001` and
  `MN_U0007` hidden where the first slot is filled), `NN_U0008` the Result
  bar under the hint's words, `cir`/`cir1` a faint ring, `light`/`light1`
  the orange and green rays, `light2`/`light3` the white rays of the
  fusion's flash, `NN_U0001` the result's shadow, `NN_U0002` the result
  weapon. `NN_U0006`, `NN_U0007`, `Seed1`, `Seed2`, `MenAltermixUp0001`
  and `MenAltermixUp0102` drew nothing before or during a fusion. The
  sparkles are `PtcCmnMenuAlterMix`, a particle model (vtable `0x00D14A8C`,
  CGFX models `0x00D14AF4`): no mesh nodes, and clearing its `+0x1F6` byte
  does not stop its draw, so it drew in every layer's replay (36 node
  layers exhausted the drawing buffer: the rows and weapon vanished).
  A replay that does not own it now takes it out of its layer list.
  The bonus slots' words are fitted to the slot width by the game; in the
  Details card's guard-band replay they come out narrower than native
  ("Melee defense +1" 68 px at full size, native 85, the unsplit layout
  107); without the guard band every word breaks. Not fixed.
  That build (top split, the bottom one whole Control) also broke mouse
  clicks on the Board: `mhover` + `press` on a weapon did nothing, where
  the Generic layout opened Fuse/Cancel. The bottom split below fixes it.
- Armory — Fuse Weapons bottom node ownership (21 Nov, same slot).
  `MenAltermixBt0000`: `NN_B0002` Weapon A's (green, top row) background,
  `NN_B0003` Weapon B's (peach, left column), `NN_U0000` a grey mask under
  the Frame that reaches past the screen's edges (drawn whole, it showed
  as grey blocks beside the Board). `MenAltermixBt0002`: `NN_B0000` the
  Frame (corner art, A and B tabs, bottom bar), `MN_B0000_` `NN_B0004`
  `MN_B0005` Weapon A's scroll bar (track, thumb, arrow), `MN_B0001_`
  `MN_B0003` Weapon B's, `NN_B0005` Back. `Bt0001` the grid, `Bt0004`
  and `Bt0005` the A and B lists (one dynamic-texture node each), `Bt0006`
  `Bt0007` `Bt0008` the column, row and grid Cursors, `Bt0009` the sort
  order bar (its words, such as "Order Used", come from the bottom text
  pass), `Bt0010` both questions (Fuse/Cancel, then "Equip?" No/Yes)
  with their words, `Bt0003` the fusion's flash. `MenCmnWindowBt0000`
  drew nothing in any phase seen.
- Fuse Weapons' phase is the word at Screen object `+0x2B20` (object
  vtable `0x00CEE148`, 0x1406C200 in that slot; written by `0x002C74DC`
  with r4 the object): 0 on entry and after Cancel, 2 after a sort-order
  touch, 3 after its direction arrow, 5 Fuse/Cancel, 6 fusing, 8 Equip?.
  The model and node flags do not change between phases; the questions
  appear by animation. A native touch on No does not answer Equip? (the
  Generic build too); Yes and A do.
- 17 bottom node layers replayed every frame froze the game in the
  loading after Yes (grey screen with the loading orb, for good); 16, any
  one fewer, did not. The question, flash and Window layers now draw only
  in phases 5..8, so the Board replays 14. With them, the 36 one-node top
  layers, and Powers' entry, this is the third time the replay count or
  size froze or blanked a Screen.
  Windows release rebuilt; `dist/zakuro-kiu-mouse.exe` and the Downloads
  copy have SHA-256
  `41d2dcb884bbc8b1b5a4bafb41fdd2b4eb30be61c18a5498609f135dcb479db8` and
  completed the five-frame Windows startup check.
- That gating still replayed 17 in phases 5..8, as the dialog's three
  layers joined the Board's 14. A mouse click on a grid cell (the second,
  as natively) set phase 5 and, on its first frame, the bottom pass's
  buffer bind 0x009166E0 called 0x00B8C030, which read guest `0x1C`
  through a null from 0x001ECC5C (the context's +8, then +0xC: no bound
  framebuffer); the bottom stopped updating, so Fuse/Cancel never showed
  (`access to unmapped guest address 0x0000001C`). Natively the game hides
  both scroll bars and Back then, so those layers now draw only outside
  phases 5..8 and every phase replays 14. Grid click, Fuse, Equip? and Yes
  by mouse on slot 4 (`trace/fuse-slot4.state`) then ran with no fault and
  no freeze. Windows release rebuilt; `dist/zakuro-kiu-mouse.exe` and the
  Downloads copy have SHA-256
  `72154f02c79ea4d1ac36d227769f4c34b0522fa63162127910089623f6b447e1` and
  completed 60 headless frames; the previous build is
  `dist/zakuro-kiu-mouse-before-fuse-dialog.exe`.
- Power Portrait (the Vault's, Scripted Screen 27, MENUID 47; slot-2 state
  `trace/powerportrait.state`): object 0x14057168 there, vtable 0x00CF9388,
  update 0x0018AEB8, a thunk of 0x001AD618 (0x0018AEC8, a thunk of
  0x001AD498, runs beside it). Found with `entered <object>`; `calls` on the
  0x001B4Cxx thunks caught nothing. Top-screen node ownership,
  `MenColPazUp0000`: `NN_U1101` Frame (with its green board), `NN_U1102`
  Checkerboard, `NN_U0001`..`NN_U0060` the 60 portrait pieces (hidden until
  earned). `MN_U1103`, `complete` and `flash` draw nothing at 15%. Owning
  each piece in its own layer lost the earned pieces' drawing; one rest
  layer keeps them. Bottom, `MenColPazBt0001`: `NN_B1000` the Current
  Completion banner and ring in one mesh, `NN_B1001` the digits; `comp`
  and `pPlane1` draw nothing. `MenCmnBg0000` is the bottom backdrop: left
  as a plain group model it drew across the whole top screen and covered
  the portrait, so it is a bottom node layer.
- Armory Weapons top-screen node ownership (found by moving one node at a
  time; hide-diffs were misleading): `MenEquipUp0100` is the paper with its
  crest, ornaments and ink frame in one mesh. `MenEquipUp0121`: `NN_U0005`
  and `NN_U0007` Description board and its lines, `NN_U0000` Category emblem,
  `NN_U0003` with `pPlane1` Stats card, `NN_U0002` Weapon shadow, `NN_U0001`
  the spinning Weapon; `NN_1111` and `pPlane2` draw nothing in
  `trace/armory-weaponlist.state`. `MenEquipUp0500` is the equip flash. Every
  drawing node must be owned: one left to the Details card draws after the
  layers and covers them. Split screens replay the Details card last for the
  words; `MenuTop::part` had dropped it (Powers lost its description too),
  and its native-viewport mapping halved the words' width, so it takes the
  guard-band projection.
- Harvested child Control identities from native hits:
  Other: `MenOptionOtherBt0001` name/49C14849, 0002 caption/F28BAA60,
  0004 itunomani/9E610C53, 9999 return/928BEAA4.
  Controls: `MenOptionControlBt0001` button/DE4F96DF,
  0002 touch/56866028, 0006 opt/FB565F88, 9999 return/928BEAA4.
  More Controls: 0004 guide/8D79D12E, 0008 lead/4998C4E7,
  0007 asis/4A0C9579, 0003 auto/4A0F894B, 0005 test/49B8FADA.
  Its mode is bit 0 of object +0x11D: clear for Controls, set for More
  Controls (confirmed in states s17/s23 and 002E19F0..002E19F8).
  Reticle/Camera: `MenOptionCtlTpenBt0001` groundBtn/E84BA0DD,
  0002 airBtn/21E46FF0, 9999 return/928BEAA4.
  Profile: `MenOptionNameBt0001` Name/4A41C1D7, 9999 Return/9A07E025.
  Profile Yes/No: `MenSelnameBt0001` BtYes/3837AABD,
  0002 BtNo/4F14658D, 9999 BtReturn/3B8915AB.
  Subtitles: `MenOptionOtherCaptionBt0001` upperButton/6A1911BF,
  0002 lowerButton/E76D363A, 0003 offButton/DD5F933C.
- Volume: `MenOptionSoundBt0002` music/6AFB5539,
  0003 se/CD949985, 0004 voice/002A0EEA,
  0005 soundTest/1C05D789, 9999 return/928BEAA4.
  Models 0001 and 0006 contain shared labels/slider pieces; keep them
  with each slider, clipped by its Placement. Native directional helpers
  002D590C/002D5B40/002D5D74/002D699C handle Up/Down/Left/Right.
  Linked directions queue the chosen name from retained kind 00D70ED8,
  using the native key-repeat flags and 002EA7A0; unlinked directions
  retain the original slider adjustment. In s18 Music's float at +0xD8
  (1403EA08) is 79: a moved slider drag changed it to 10; holding a Left
  self-link for 40 frames left it at 79.
- SpotPass Dialog: native object 1404F4E0 in s27, choice at +0x44
  (1404F524): No = 0, Yes = 1. Both hit BtDialog/B1EBC922; distinct
  elements, not distinct model hashes. Split its caption and choices by
  native source rectangles and crop the shared MenCmnWindowBt0000 model.
  The store at 00241368 keeps native repeat/lighting/activation; LR
  002412A4 identifies Left and 0024130C Right. Native Up/Down do not
  select a choice, so the editor reports those axes unavailable.
  Both horizontal self-links passed Cursor assertions; a moved No at
  native (80,190) dismissed the Dialog without enabling SpotPass.
- The Vault's Idol Toss is the hub's screen again, its object at 0x14041518
  here and its update 0x001B4D4C, not the hub's 0x001B4D58: the same class
  dispatches by event, so both thunks are in MENU_SCREENS. Its Back is the
  whole strip below y 200.
- Its other screens are chrome over a viewer: the Idols viewer (object
  0x14040508 in its state, vtable 0x00CF93D8, update 0x001B4D70), the
  Music Gallery's list (object 0x1403E938, vtable 0x00CFB010, updates
  0x001B4D28 and 0x001B4CF8), Power Portrait, Records and AR. They have no
  outline state of their own: a tap selects (Music Gallery's rows and its
  play mark), a tap on Back leaves, and the pointer's press is that tap.
  Their `+0x44` is not a part's hash (the Idols viewer's is a constant
  0x928BEAA4) and 0x00345C18 does nothing on them.
- The Music Gallery's list is the screen's own, at +0x190 (0x1403EAC8 in
  that state): the track it has picked an index at +0xE4, the first row it
  shows at +0xEC, its rows at +0xD0 and the ones it shows at +0xC8. Its
  keys go through 0x0031AA88(list, down, move): with the third word 0 it
  moves the first row by one (0x002E5B80(list, rows)), with 1 it moves the
  picked track, down = 1 up = 0 - called by hand it took the pick 0 to 5
  and back - but the rows it shows did not come back with it and the probe
  left fewer of them showing, so the third and fourth words the keys pass
  are not known yet and the wheel is not wired to it.

- Screens whose parts act on touches, not on A (Idol Toss, Treasure Hunt),
  take no click of their own: `CLICK_TOUCHES` in their MENU_SCREENS entry
  keeps them out of `menu_clickable`, so a press over a part becomes the
  touch at the pointer, as for unmapped controls.

## Screen anchors

Canonical Screen names are in `GLOSSARY.md`; this table is the only place their code anchors live. Scripted Menu rows come from `MENU_SCREENS` in `crates/zakuro-core/src/aim_mouse.rs`; `vtable` is the object's first word.

| Screen | vtable | update | outline | A / use |
| --- | --- | --- | --- | --- |
| File Select | 0x00CEC3F4 | 0x002A3EC4 | outline_file | 0x003374D0 |
| Chapter Select | 0x00CF243C | 0x002A3FCC | outline_chapter | 0x003675C0 |
| Main Menu | 0x00CFBD9C | 0x002A3F9C | outline_buttons | 0x0036F59C |
| Options | 0x00CEAE98 | 0x00224BA0 | outline_queued | 0x002FB9E0 |
| Other | 0x00CF2368 | 0x001ECB4C | outline_other | 0x002D50FC |
| Controls | 0x00CF84DC | 0x00224C18 | outline_controls | 0x003186E8 |
| Reticle/Camera | 0x00CF9B60 | 0x001B4C74 | outline_queued | 0x0029B26C |
| Subtitles | 0x00CFA034 | 0x001ECBBC | outline_subtitles | 0x002E5448 |
| Volume | 0x00CF2390 | 0x00224BD4 | outline_volume | 0x002D56CC |
| Profile | 0x00CF8E8C | 0x001ECBF4 | outline_profile | 0x002E2F4C |
| Profile Yes/No | 0x00CEC41C | 0x0016756C | outline_yes_no | 0x001C16D4 |
| SpotPass Dialog | 0x00CF8270 | 0x002A4070 | outline_dialog | 0x00314DF8 |
| Solo Hub | 0x00CFC3D0 | 0x002ECCF4 | outline_solo | 0x00370300 |
| Arms Altar | 0x00CFCA28 | 0x002A1FD8 | outline_altar | 0x00322C34 |
| Armory — Gear | 0x00CEC2B8 | 0x002B6DB8 | outline_armory | none (list input) |
| Armory — Redeem Hearts | 0x00CEFCA8 | 0x00238CBC | outline_armory | none (list input) |
| Armory — Convert to Hearts | 0x00CEEBFC | 0x001C9734 | outline_armory | none (list input) |
| Armory — Fuse Weapons | 0x00CEE148 | 0x00230B5C | outline_armory | none (list input) |
| Armory — Powers | 0x00CEC2E0 | 0x002BB14C | outline_armory | none (list input) |
| Armory — Weapons | 0x00CEE1E8 | 0x0019CDC8 | outline_armory | none (list input) |
| Armory — Categories | 0x00CEE1C0 | 0x0019BDC4 | outline_armory | none (list input) |
| Practice Range | 0x00CF0154 | 0x0023A9FC | outline_practice | 0x0030B5B4 |
| Idol Toss (Solo Hub) | 0x00CF9400 | 0x001B4D58 | outline_idol | none (click is a touch) |
| Treasure Hunt | 0x00CEEDD8 | 0x0018AF44 | outline_treasure | none (click is a touch) |
| Vault | 0x00CF8248 | 0x00224B80 | outline_vault | 0x00345FDC |
| Idol Toss (Vault) | 0x00CF9400 | 0x001B4D4C | outline_idol | none (click is a touch) |
| Idols (Vault) | 0x00CF93D8 | 0x001B4D70 | outline_none | none (click is a touch) |
| Power Portrait (Vault) | 0x00CF9388 | 0x0018AEB8 | outline_none | none (click is a touch) |
| Control Configuration | in-game Options page 5 | 0x00A0ACF0 | none | drags what it holds |
| Pause | object at ctx+0x243C | 0x0060DC7C input | pause options page at +0x678 | touch: 0x00A0867C main page, 0x00A06188 Options |
| Retreat Dialog | dialog object | 0x007226C4 input | picked button at +0x294 | touch began now |
| Music Gallery | 0x00CFB010 (list) | 0x001B4D28, 0x001B4CF8 | none | keys 0x0031AA88 |
| Fiend's Cauldron | pot task 0x00CF9AEC | 0x00251624, 0x00251C40 | cursor class 0x00CEE170 | touch |

## The Fiend's Cauldron

States: `trace/s10.state` (your 9.state of 29 Oct, the bar), `s11` chapter
select just before it, `s12` the bar, `s13` just after Move Out (your 1-3
of 29 Oct, 19:06).

- Its 2D is not the battle UI's: nothing goes through the sprite batch
  flush (0x0092F408) or a 2D pass start (0x0092E4F8) but the dialog
  context's corner icon. It is drawn by the scripted layout system.
- Bottom screen boxes, from a screenshot (bottom pixels): "Try This?" tag
  y 0..12 above the bar, bar x 22..322 y 13..30, the skull dial y 40..80
  sliding along the bar with the value, pan and pot x 70..250 y 75..190,
  the bet counter x 218..310 y 143..170, Back x 10..45 y 205..238, Move Out
  x 60..250 y 185..240. The top screen's marker stem meets the bar at the
  same value 40 px to the right (top x = bottom x + 40), as for the pause
  menu.
- 0x0067E468 (vtable 0x00D00248, +0x10) binds a parameter table of the
  screen's parts by name (startBody_ "GO" = Move Out, returnBody_ "return"
  = Back, level_bar_ "Level_Bar", saturn_pot_ "Saturn_Pot", go_uranai_,
  saturn_dish_, ...): an object at 0x14B91158, nothing calls into it per
  frame. Animation entries naming the parts (vtables 0x00CFC4DC,
  0x00CEC7D0) sit at 0x14914668.. with an owner 0x14050A50, also not used
  per frame.
- Per frame, the screen runs as tasks from the task loop (lr 0x00417680).
  Only here, against states 1-9 (`ran` in the harness): tasks with vtables
  0x00CEE170, 0x00CF7344, 0x00CF9310, 0x00CF9338, 0x00CF9AEC, 0x00CF9FE4,
  0x00CFA00C, 0x00CFA330 (objects 0x14051270, 0x1406A950, 0x1406C358,
  0x14063858, 0x14045F68, 0x14051C30, 0x1404B0A8, 0x140554D8). Which of
  them are the cauldron's and which any menu between chapters has is
  still to tell, with a state on another menu (chapter select).
- Telling it's shown (the fork's `AimMouse::cauldron_shown`): the pot's
  task (vtable 0x00CF9AEC, 0x14045DB0 in s12) runs 0x00251624 every frame
  while the bar is up, and 0x00251C40 while it pours after Move Out (its
  update pointer moves on); neither runs on chapter select, after the
  pour (a black fade, then the battle loads) or in battle (states 1-9,
  11). Checked with the harness's `shown`: on in 10, 12, 13, off in 11, 4
  and 5, on for about 90 frames of the pour from 13, then off.
- While pouring, the pan lifts to the top of the bottom screen (y 10..70)
  and the bet counter with it; the bar is gone and the top screen fades.
- The loop at 0x00A35750..0x00A35C1C, which draws through the 3D renderer
  (0x00F2DC9C) with a projection for the top (0x00DB72D0, once for each
  eye at x +-18.3) or the bottom (0x00DB7290), draws only the text lines
  ("fnt_%02d"): with its draws (0x00909ACC at 0x00A35B00, 0x00A35BC8)
  skipped, only the hint line is gone. Everything else is models in the
  scene, below.
- Tried and dropped: composing the window from crops of the two screens'
  pictures in the presenter. It works but looks pasted together (cut
  edges, boxes of the bottom screen's background).

How the screens are drawn (all from `trace/s12.state`):

- A frame renders render passes (vtable 0x00D16058, run by 0x00BA9E14):
  "PreRenderPass", "TopRenderPass", "BottomRenderPass", each a list of
  drawings at +0x40, +0x50 how many, the screen at +0x58 (-1, 0 top, 1
  bottom), started by 0x009167C0 (0x00916588 picks the buffer: 0 the left
  eye, 1 the bottom, 2 the right eye). Top and bottom draw the same scene
  (0x149A6378) with different cameras.
- The scene: cameras by screen from +0xC8 (0x00E14BEC top, 0x00E14DCC
  bottom, both perspective lenses), layers 0x28 bytes each from +0xEC,
  each with a list (+4) of models at +0x18..+0x1C. A model (vtable
  0x00D14AF4) has its CGFX model (CMDL) at +8, its runtime meshes after
  it (0x88 each), and its mesh nodes' visibility at +0x17C (8 bytes each,
  shown at +4), in the CMDL's +0xD0 dictionary's order. 0x008F5FB4(model,
  mesh) is the visibility test; the CMDL's own flags are not read.
- The top's drawing (vtable 0x00CEF1A0, 0x0072A034): 0x00727804 sets the
  viewport from +0x44 (x, y, w, h, the buffer turned: x and y count from
  the screen's right and bottom) and the camera, then 0x00727C24 draws
  layers 0xB..0xE, each the right eye, the left eye, then its text
  (0x00718120 at 0x00727F10; the hint is layer 0xB's). Layer 0xB: the
  picture, one model `MenBetheartUp0000` (nodes: polySurface45 the dark
  frame around the board and the hearts' box, polySurface51 the shading,
  polySurface47 the intensity panel), the hearts' number and icon.
- The bottom's drawing (vtable 0x00CF6D6C, 0x007C8ECC): the viewport, the
  bottom camera, layers 0xF..0x12 when +0x5D is set (loop ends at
  0x007C91C4), then the bottom 2D. Layer 0xF: `MenBetHeartBt0003` pot,
  `Bt0004` pan and hearts bet, `Bt0002` bar and dial, `Bt0005` "Try
  This?", `Bt0006` crown, `Bt9999` Back, `Bt0010` the ring background,
  `Bt0007` glow, two particle models; layer 0x12: `Bt0000` Move Out
  (nodes `button`, `cursor`, the red outline).
- A camera's projection is kept at +0x6C while bit 2 of +8 is set: its
  first row gives the view up the screen, the second across it from the
  right. Scaling and shifting them shows a part of the screen in the
  viewport. The top's stereo camera (0x00916234) rebuilds each eye from
  it and swaps where the shifts land (see `cropped_projection`).
- Mirroring a model by the projection turns its triangles' winding, and
  the materials' face culling (GPU register 0x40, `00010040` after the
  value in the CMDL's commands) then drops them.
- The pot task's +0x6C is the intensity (float). The dial on the bottom
  screen is at x 33 + 28.3 per 1; the top's panel slides under the board,
  its middle at x 64 + 30 per 1.
- The cursor (what the arrow keys light): a task, vtable 0x00CEE170,
  updated by 0x0027B100. It names what is lit at +0xD8 (hash, string):
  `Level_Bar` 0x53D3F0F1, `GO` 0xCD94A288, `return` 0x928BEAA4. Up and
  down go between the dial and Move Out, Back to the dial; left and right
  between Move Out and Back, and move the dial when it is lit. A key
  lights another with 0x003062F0(cursor, name), 0x00305D44(cursor, 1),
  then +0x191 |= 0x10; done before the key handling (at the update's
  entry) the old one stays lit, done at 0x0027BBA0 (after it) it works.
- Move Out acts on the press, not the release, and grows when lit.
- Your cauldron states were saved with the 16:9 widening in both lenses
  (aspect 1.7778 and 1.4222, the game's are 1.6667 and 1.3333): harness
  scripts write the game's back after loading.
- "Not enough hearts": the bar model `MenBetHeartBt0002`'s `noheart` node
  (shown while the bet costs more than there is) and one text line drawn
  by the bottom pass's text call (0x007C912C, through the font hook at
  0x00A36FA4). The text's anchor is projected through whatever camera
  crop a replay uses, so before the fix it showed up in Rest's and Move
  Out's boxes. It is now drawn only with the dial.
  Windows release rebuilt as `dist/zakuro-kiu-mouse.exe`, SHA-256
  `06c6d27c7c16c68d520aebc1c5eddfb7f782b8090a19ce07a580f790c38fd098`; the
  previous build is `dist/zakuro-kiu-mouse-before-cauldron-tag.exe`.
- Top camera crops (`cropped_projection`, top): the stereo submit moves
  each projection row's shift, as written, onto the other axis. Shifts
  were therefore wrong whenever the source window was not the full height
  (vertical) or the screen was widened (horizontal): the panel lost its
  rounded bottom and drifted with the window's aspect. Each middle is now
  scaled by its own row's share over the other's. Checked with crops of
  the heart icon at 16:9 and 21:9 (`trace/s12.state`, lens aspects reset
  with `poke 0xE14BC8 1.6666666` / `poke 0xE14DA8 1.3333334`, since the
  state was saved widened): every box within 1 px of the expected one.
  The widening a crop divides by is now read from the camera's own lens.
  The 55 save states in `trace/` give identical top screens with the old
  and new top crop, so the Scripted Menus' top crops are unchanged.
  The default panel window is now the whole panel, `[-6, 170, 140, 72]`.
  Windows release rebuilt as `dist/zakuro-kiu-mouse.exe`, SHA-256
  `068a04db22ddedfed77e88aef11e63e08c043961d221373822cbf95445205126`; the
  previous build is `dist/zakuro-kiu-mouse-before-crop-shifts.exe`.
- Cauldron parts are no longer cropped: each is drawn through the whole
  top screen with its camera moving and scaling it (`Placement::whole`),
  isolated by models and, for the shared bar model `MenBetHeartBt0002`, by
  mesh node (rendered one at a time: `base` the bar, `num` its numbers,
  `ber` the dial and its stem, `cursor` its lit outline, `noheart` the
  tag). The board keeps its window, the mask its hidden frame gave: without
  it the strip's neighbouring boards show. Windows release rebuilt as
  `dist/zakuro-kiu-mouse.exe`, SHA-256
  `37fde4aa7345bc6aadeed34fc62f9536c1027daa02579eae9cdd33ed00ff27b2`; the
  previous build is `dist/zakuro-kiu-mouse-before-uncropped.exe`.
- The hint line ("Bet hearts to…") never went through the font hook the
  old 0.7 scaling sat in: it is drawn by the text loop at 0x00A35750,
  whose camera calls (0x00A35AF4, 0x00A35BBC, 0x0090A5B4 with the
  projection in r1, 0x00DB72D0 for the top) take a placement as the
  dialog's words do. It is now the ninth Cauldron part, Hint, placed
  there (`cauldron_hint`). The flag marking layer 0xB's text was being
  cleared when the game came back into `TOP_TEXT` after its viewport
  call; it is now set on the first entry only. Windows release rebuilt as
  `dist/zakuro-kiu-mouse.exe`, SHA-256
  `d9a5ada9476db4faef1ecec888502c53bd66921826d856332313b4d1ee723a5b`; the
  previous build is `dist/zakuro-kiu-mouse-before-hint.exe`.
- Menu editor: the arrow keys nudge the selection (1 pixel, 10 with
  Shift) while no text field is focused and no drag is active. Dragging
  prefers any already-selected item under the press over a covering one,
  so an item picked from the list pane can be dragged without moving what
  covers it. Windows release rebuilt as `dist/zakuro-kiu-mouse.exe`,
  SHA-256
  `bc9194fbfb58c5d844710a0563a407a6ac4d1771d237e8dd62a013b895fdf7b7`; the
  previous build is `dist/zakuro-kiu-mouse-before-nudge.exe`.

## Weapon effects: geometry shader mode 2

Reported with missing Sagittarius Bow and orbital effects and repeated
`geometry shader mode 2 is not implemented` warnings. In Zakuro's
`zakuro-gpu/src/raster.rs`, `geometry_stage` returned no vertices for every
nonzero mode, dropping those draws entirely.

Mode 2 is fixed-primitive input through float uniforms. Register 0x252
holds the mode in bits 0–7, vertex count minus one in bits 8–11, attribute
stride minus one in bits 12–15, and starting uniform index in bits 16–23.
Each complete group of vertex shader outputs fills that uniform range
and invokes the geometry shader. This differs from mode 0, which fills
input registers. Semantics cross-checked against Azahar's
[pipeline registers](https://github.com/azahar-emu/azahar/blob/master/src/video_core/pica/regs_pipeline.h)
and [geometry pipeline](https://github.com/azahar-emu/azahar/blob/master/src/video_core/pica/geometry_pipeline.cpp).

Implemented that path with bounds/stride validation and b15 set after each
invocation within the draw. A rendering regression sends three vertices
with two attributes each to c4–c9, twice with different colors, and checks
the final framebuffer. Both that test and the existing point-mode test
pass. The full Vulkan GPU suite passed 178 tests, with 2 existing benchmarks
ignored. Actual Sagittarius Bow/orbital effects still need an in-game check.

Windows release rebuilt as `dist/zakuro-kiu-mouse.exe`, SHA-256
`dcaa63994cfec8b4b4c9f02e4f1aba02b17d2a57065a0705003d418eedeaea09`.
The previous build is `dist/zakuro-kiu-mouse-before-geometry.exe`.

## Scripted menus on the top screen

The top drawing's common entry after its profiling scope is `0x0072A050`;
`0x0072A0B4` joins its normal and special scene paths. Hooking only
`0x0072A09C` misses subtitles and several other pages. `aim_mouse/menu_top.rs`
uses that common pair to place the native top information, then replay the
bottom's model groups. Original model order determines overlay order, so a
selected hub button keeps its native foreground position. Lists and boards
use one transform, shared with the pointer inverse. Inverse coordinates snap
within 0.0001 of an integer before touch conversion, avoiding a row changing
from 84 to 83 through floating-point roundoff.

During menu entrance animations, hub models can appear before the menu's
input-update hook starts. The renderer recognizes the Solo, main, Arms Altar,
Practice Range, Vault and Options hub models during that interval and uses
their normal layout; input remains disabled until the menu update identifies
its owner. In the main-to-Solo trace, Solo models first appeared 21 frames
before its input update.

The existing bottom drawing stop at `0x007C91C4` avoids replaying native
bottom 2D/scenery. Individual hub controls use model-name allowlists; their
labels are baked into their meshes. Dense pages keep native text and controls
together. The text scissor call at `0x009164A8` and return at `0x00916580`
map clipping through the same transform. Font calls in `0x00A36E64`'s
function temporarily shrink the Cauldron's hint font and restore it afterward.

PICA registers `0x65..0x67` provide include scissoring. The GPU previously
ignored it; include rectangles now intersect the existing raster bounds for
both software and Vulkan. A shifted/oversized viewport must not enlarge the
framebuffer's tile stride: framebuffer dimensions remain authoritative.
Regression tests cover clip color/depth writes and offset viewport rendering.

Drawing dense bottom lists twice overflowed the game's command allocation;
only the left-eye replay is made. The 3D is always off now, so there is no
stereo fallback any more. The Cauldron's
existing drawing path remains separate. `ZAKURO_MENUS_TOP=0` disables this
feature without changing console state or the save-state format.

Headless captures: `trace/one-screen/{native,wide,ultra,narrow}/`. There are
29 scripted layouts plus the camera and generic dialog using existing pause
rendering. Tests include native/top hover, activation, keyboard handoff,
weapon/power scrolling, sliders, and held power dragging. Windows pointer
acceptance is still manual.

The final native/top input comparison matched all 112 recorded state words
and all 21 accepted clicks. The held power drag initially differed because
the fixed two-vblank initial touch could be missed during a replay. Keeping
that initial pixel through the measured drawing interval fixes pickup;
the resulting dropped-board screenshot is now byte-identical to native
input. Core tests cover that interval and exact native-pixel round trips.
The final core mouse suite passes 32 tests; the GPU suite passes 181 tests
with two existing benchmarks ignored. High-resolution Vulkan captures of
Solo, fusion, powers and a dialog are in `dist/one-screen-menu-screenshots/`.

Windows release rebuilt as `dist/zakuro-kiu-mouse.exe`, SHA-256
`4b61ca0642451256c3aa8378bd189693b6c6f1d6bf55d02c3f54ac48bfe089be`.
The previous executable is `dist/zakuro-kiu-mouse-before-one-screen-menus.exe`.
Screenshot gallery: `dist/one-screen-menu-screenshots/index.html`.

Validation limit: a single Vulkan process capturing Solo and then loading
fusion stalled during the second warmup. Fresh processes captured each of
Solo, fusion, powers and the dialog successfully at 3x. The four full layout
matrices and native/top input comparisons completed with software rendering.
Repeated state loading after high-resolution Vulkan rendering needs follow-up;
it is not covered by the passing single-state captures.

## Solo hub button collisions in the resized layout

The interactive buttons use their own 2D rigid-body system, not just the
menu hit-test triangles. `0x00D48E70` points to its body list (`+0xCC` next).
Each body points to its layout part at `+0x50` (part name at `+0x24`) and
native element at `+0x54`. Position is `+0x74`, vertices are linked from
`+0x58` (`+0x48` next, previous/current world points at `+0x1C/+0x28`),
and drag/spring joints are linked from `+0x64` (`+0x6C` next, world anchor
at `+0x58`). Restoring the joint anchors matters when borrowing coordinates.

`0x002F0770` resolves body pairs. Its calls at `0x002A3374/0x002A3470`
now borrow a common canvas coordinate space and restore native coordinates
on return, preserving solver changes. The common uniform scale cancels,
so each body needs the translation from its displayed placement divided by
that scale. The wall solver call at `0x002A34C8` receives expanded bounds.
The solver itself stays recompiled.

The drag joint is input-owner `+0x600`, selected body `+0x5FC`, target at
`+0x5D8`. Its old screen-to-world conversion and touch event filtering stop
long PC drags at the original screen. Touch callbacks and the physics update
(`0x0025EF84`, after its register push) maintain the virtual target from the
host pointer, with the physical HID touch kept in its valid 320x240 range.
This is scoped to Solo while one-screen menus are enabled.

Validation: a short Practice Range drag leaves the distant Arms Altar body
at x=9; a drag reaching its visible location moves it to approximately
x=12.33. The long drag reaches native target x=52.19, beyond the old screen,
and both contact and no-contact cases use physical host-pixel paths.

## Chapter select scene separation

Chapter select (menu index 1) keeps its native top scenery at full-window
size. `MenStageselectUp0000` contains both scenery and the chapter artwork:
`NL_U0000` is the backdrop, `PN_U0000`/`PN_U0001` the corner decorations,
and `PN_U0002` the chapter picture. `MenStageselectUp0001` supplies the
completion badges. Replay the picture, badges and top text proportionally
at the top, with scenery and its text pass omitted from that replay.

The bottom overlay retains `MenStageselectBt0000` (chapter thumbnails),
`Bt0002` (scrollbar), `Bt0003` (difficulty controls), `Bt0004` (completion
marks), and `Bt9999` (Back). Hide `Bt0000`'s `NN_B0000` mesh as well as
`Bt0001`; otherwise a white bottom-screen panel follows the controls.
The native chapter scrolling clips remain mapped with the control group;
background and details do not accept input. All visibility/material edits
are restored before native bottom rendering.

Chapter hover must preserve its last completed native hit until the host
pointer moves. The list scrolls when a thumbnail is outlined; repeatedly
refreshing the same hit every frame invalidated `menu_looked` at the time a
click arrived. The chapter-specific refresh exception preserves the native
menu's selection and leaves animated hub-model hit refreshes unchanged.

Checked headless on `trace/s11.state` at 5:3, 16:9 and 21:9
(`trace/chapter-layout-verify.txt`): the second thumbnail is outlined and the
list scrolled to it, Back takes the return hash `9A07E025`, and a click at
`mfrom 140 60`, inside the chapter picture, is taken and opens the Fiend's
Cauldron (`trace/chapter-click-verify.txt`).

Windows release rebuilt as `dist/zakuro-kiu-mouse.exe`, SHA-256
`072a802ccb0308459c6112b4580a16e5c19f4a7d97e3a7aad892fa9e643f7756`,
and copied to `C:\Users\Patrick\Downloads\zakuro-kiu\zakuro-kiu.exe`.
The previous executable is `dist/zakuro-kiu-mouse-before-chapter-layout.exe`.
Manual Windows testing remains.

Entrance: while chapter select slides in, its update has not run yet, so
`menu_top::scene` found it only by model name. `MenStageselect*` fell to the
generic native fallback (kind 8): for about 20 frames after A on the Solo
hub, the top showed the two native screens side by side over black, then
snapped to the layout. `MenStageselectBt` now maps to index 1 the way
`MenSelfileBt` maps to File Select, so the slide-in plays inside the final
layout from its first frame (Solo hub from `trace/s11.state` + B, then A,
shot every 3 frames).
Windows release rebuilt as `dist/zakuro-kiu-mouse.exe`, SHA-256
`cba7c34a0c1116df7df93c20eb62abdd27e298b9af8c6cae0ed7fec53996ec58`; the
previous build is `dist/zakuro-kiu-mouse-before-chapter-entry.exe`.

## Chapter Select List as one row

The List (`MenStageselectBt0000`) is one model with a skeleton: model
`+0x22C` is the skeleton, its `+8` the CGFX SOBJ, whose bone DICT
(`SOBJ+0x1C`, relative) names the bones in the order of the world matrices
at skeleton `+0x40` (3x4, 48 bytes; locals at `+0x20`, 4x4 worlds at
`+0x30`). Bone 0 is the root, 1 `ML_B0000`, 2 `ML_B0001`, and so on, with
`ML_B0009_0/_1` and `ML_B0023_0/_1` as children. Translation is `+0xC`
(across) and `+0x1C` (up): each chapter is 8 units further along, the root
at -8 per chapter outlined, and the zig-zag is 6.4 / -2.6. Measured against
bottom-screen captures, a bone at (x, y) is drawn centred at
(163.75 + 8.75x, 123.9 - 10.22y). The game recomputes these every update,
so writes to them never reach a draw. Every open chapter's node stays shown
wherever it is scrolled; only the native List window cuts them.

The laurels are one `MenStageselectBt0004` model per chapter, each with its
world matrix at model `+0x8C` (translation `+0x98`, `+0xA8`) placed on its
chapter (uncleared ones at y 100). `MenStageselectBt0003` is the red cursor,
kept on the outlined chapter. The chapter object (0x14040DA8 in
`trace/chapters-many.state`) keeps the scroll target at `+0x178` (-8 per
chapter).

Each open chapter is a Row Part: its source is 144x90 around its bone's
centre, read every frame, and its slot is in one band above Back. A Row
replays only its own `ML_B` node, its own laurel and, when outlined, the
cursor, and no scissor at all: a cut to the slot cropped the laurel (which
overhangs the thumbnail's left edge) and the cursor, and the native List
window hides chapters scrolled off its screen. Rows skip the bottom text
pass (chapter labels are in the thumbnail textures); uncut, it drew the Gear
hint enlarged across the screen once per Row.
Replaying the whole List in every Row blacked out the frame by the fourth
Row (the drawing-buffer limit seen on Armory). The editor layers before the
Rows are cut from the two screens' drawings (`CHAPTER_LAYERS`): Title,
Picture and Stats from the top (the rainbow band, rule and grey board behind
them are the backdrop pass), Back and the Gear Hint strip from the bottom;
the native thumbnails, laurels and scrollbar are not drawn in them. Their
defaults reproduce the earlier whole-screen placement exactly. Layout files
saved before the split index Chapter Select's layers differently and need
re-editing. Hovering a Row picks its chapter by `PICK_CHAPTER` directly (the
native hit test cannot reach a chapter scrolled off the bottom screen);
native thumbnail hits are ignored. The wheel moves only the row's own scroll.

Checked headless on `trace/chapters-many.state`: at 16:9 all five chapters in
one row with laurels and the cursor on Chapter 5; `mhover 112.5 190` outlines
Chapter 2 (`+0x12C` = 1) and the details follow without the row moving; the
click is taken and opens the Fiend's Cauldron. At 1:1 two Rows fit:
Chapters 4–5 shown, `wheel -5` shows 1–2 with the Outline still on 5, and
LEFT outlines 4 and pulls the view to 3–4.

## Button prompts in the stand-in system font

Hint strips and dialogs write the console's button symbols as Private Use
characters of the shared system font: `U+E000` A, `E001` B, `E002` X,
`E003` Y, `E004/E005` L/R, `E006` +Control Pad, `E052..E055` L/R/ZL/ZR,
`E073` HOME, `E077` Circle Pad, `E079..E07E` pad directions. French text
also uses `E068..E06A` for the ordinal superscripts "er", "re", "e". Seen in
the game's data: `"\uE000 OK   \uE001 Cancel"`, `"\uE052/\uE053: Rotate"`,
`"\uE073HOME"`, and file select's `\uE073HOME / go to HOME Menu` strip.

Without a dump, Zakuro's generated stand-in font (`services/shared_font.rs`)
had only JIS, Latin and punctuation, so every one of those characters
missed its CMAP lookup and the prompt lost its button. The stand-in now
draws them (`services/prompts.rs`), each button as a `Label` set through
`System::set_prompts`: the console's look, a keycap, a mouse button, or a
controller's button (Xbox letters, or PlayStation shapes by controller
name). The frontend picks labels from the last-used device and current
bindings, and redraws on rebinding or device switch. A redraw rewrites the
mapped block in place, only when its headers match a fresh build, so a
dumped font or an older state's differently-sized stand-in is left alone.
Symbol advance is fixed at 24 so already laid-out lines stay valid.

Validation: fresh boot to file select shows the HOME symbol with the
stand-in, matching the dump's placement; loading that state then switching
to keyboard labels showed an `Esc` keycap in the same strip two frames later.

Windows release rebuilt as `dist/zakuro-kiu-mouse.exe`, SHA-256
`442ba2bdf5e0c8cb41f1952fe21ead02ef4739800ecb5de109d5818632ad5e75`.
The previous executable is `dist/zakuro-kiu-mouse-before-prompts.exe`.

## Resizing on a placed menu froze the old width's projection

Widening the window on Chapter Select (or any Screen whose parts are drawn
through a placed camera) left the top screen's 3D at the old width: the
backdrop, picture and title stretched past their Placements and were cut by
their scissors. Entering the Screen at the final width was fine.

The top camera (`0x00E14BEC`, lens `0x00E14BB4`) caches its projection at
`camera+0x6C`; bit 2 of `camera+8` marks it built. On a placed menu a
placed projection is in the camera whenever the getter `0x003901C4` runs
(the game's own pass is placed through `menu_top.native`, parts through
replays), so the game's matrix lives only in the copies kept for
`put_back`. A width change makes `widen_view` write the lens aspect and
clear bit 2; the game rebuilds `+0x6C` for the new width, then `put_back`
writes the kept old-width matrix over it. Bit 2 stays set and the lens
aspect matches what was written, so nothing rebuilds it again.

`crop_camera` now records each placed projection, and `put_back` leaves one
the game has since replaced. Deferring the lens change until no projection
is placed does not work: on these Screens one always is.

Validation: `trace/chapters-many.state`, `wide` 1.78→3.03, 3.03→1.78 and
1.0→2.33 each leave the lens following the width and `+0x6C` built from it
(before: built for the old width). Entering from `trace/solo.state` at 16:9
then widening to 3.03 now draws the title, picture and backdrop as an entry
at 3.03 does. Regression test
`a_projection_rebuilt_for_a_new_width_is_not_put_back_over`.

The previous executable is `dist/zakuro-kiu-mouse-before-resize-projection.exe`.

## Chapter Select's cuts were measured from a squeezed picture

Even entered at the window's width, Chapter Select cut the picture's torn
left edge, the tip of the Back arrow and Chapter 1's card. The Title,
Picture and Stats are not separate assets: each part replays the whole top
drawing (`MenStageselectUp0000` without its `PN_U0002` background, plus the
font text) and its `CHAPTER_LAYERS` rectangle is what keeps it from showing
the others. Back is a rectangle of the bottom drawing. Rows draw only their
thumbnail's nodes, unclipped, from a source centred on its bone (now `ListLens`).

Those rectangles and `chapter_center`'s scale were measured from renders
whose lenses were already widened: a state saved at one width and loaded in
a fresh process makes `widen_view` take the widened aspect as the game's own
(`trace/chapters-many.state` gives the top lens 1.90 rather than 5:3). The
art there is narrower than it really is, so its true extent overran its
rectangle, and the Rows' sources sat beside their art.

From a clean render (entered from `trace/solo.state` by navigation at 5:3):
picture x 57.7–355, Back arrow x 7–48, and the List's cards at x
`160.7 + 9.68 × bone x` (Chapter 1/2/3 at −1.0, 76.4, 153.9 for bones
−16.7, −8.7, −0.7), not `163.75 + 8.75 × x`. The top parts are now full-width
bands (Title y 18–66, Picture 66–166, Stats 166–240) and Back reaches x 0;
`cut` keeps each part's art where it was, so only the window grows.

Validation: a placed 5:3 render from that clean state, with the picture's
left border, the arrow's tip and Chapter 1's frame each found where the
clean art puts them; red with the old constants (picture border missing,
arrow from its box edge at 131.3, Chapter 1 from x 0), green after. At 3.03
with cleared Chapter 2 selected, its laurels, the picture, the three cards
and Back are whole.

Loading a state saved at another width still compounds the widening; not
fixed here.

Windows release rebuilt as `dist/zakuro-kiu-mouse.exe`, SHA-256
`3cbce89304341a358a1d3144f4a75a6565860cba24d591368376922911a6d05e`.
The previous executable is `dist/zakuro-kiu-mouse-before-resize-projection.exe`.

## Chapter Select Rows slid as the game scrolled its List

Hovering a Row picks its chapter, and the game then glides its own (hidden)
List to it, the root bone 8 units per chapter. Each Row's source follows its
bone, so the art should stay put in its slot, but `chapter_center`'s fixed
`160.7 + 9.68 × x` was not where a Row's placed drawing puts a bone: the art
slid 1.2 native top pixels per chapter scrolled (5 across Chapters 1–5 on
`states/5.state`, about 25 at 1080p), back and forth with every hover.

The bottom camera (`scene + SCENE_CAMERAS + screen × 4`) has its 3x4 view
at `+0x3C` (here only z − 40) and its projection at `+0x6C`, turned like
the top's: row 0 goes up the screen (3.33 × y), row 1 across from the right
(−2.051 × x, already widened). A Row's drawing goes through
`cropped_projection`, which undoes the lens's widening (`lens_widen`, 1.218
here), so a bone draws at `160 + 9.99 × x`. `ListLens` now reads the camera
each frame, before any Row crops it, and projects bones and laurels through
it. Rows at every outlined chapter and every frame of the game's glide are
then still within 0.01 px. There was no frame lag; the scale was all of it.

The wheel scrolled whole Rows, so each notch jumped. The row's scroll is now
fractional: the wheel and keys set a target and the shown scroll eases 30%
of the way each frame, so a Row glides about 10 frames. A Row part-way in at
either end is placed too, and every Row that starts inside the window is
shown: at 16:9 five fit whole and the sixth peeks in at the right edge. A chapter outlined by hovering its Row no longer
pulls the row, so a half-shown Row does not slide out from under the mouse.

## Native Options inventory (settings plan, slice 1)

Screenshots of the saved states with `menutop 0` (13 Nov), bottom screen:

| State | Screen | Controls |
| --- | --- | --- |
| `s15` | Options | Controls, Volume, Other, Back (artwork labels) |
| `s16` | Other | Profile, Subtitles, SpotPass On/Off |
| `s17` | Controls | Control Configuration, Reticle/Camera, More Controls |
| `s18` | Volume | Music, SFX, Voice sliders, To Music Gallery |
| `s19` / `s25` | Profile / Profile Yes/No | Player name and birthday |
| `s20` | Subtitles (Pause, in battle) | Upper, Lower, Off |
| `s21` | Reticle/Camera | Air Battle, Land Battle |
| `s22` | Control Configuration | "Step 1: Choose how to move Pit": Circle Pad (Default), and a second scheme drawn as `????` |
| `s23` | More Controls | Guide Arrow, Shot Homing, Aim Assist, Autofire (On/Off), Try It! |
| `s26` | Reticle/Camera — Air Battle (Pause) | speed Slow–Fast, Stop Quickly–Slowly, Invert Vertical Movement, Invert Vertical, Invert Horizontal, OK |
| `s27` | SpotPass Dialog | No, Yes |

So the game already owns vertical/horizontal inversion, aim assist, autofire
and the movement scheme; PC Preferences must not duplicate them. The
Options main page's category labels are artwork in their models, and so
are the Row labels on Other, Controls and More Controls (texture atlases;
see "Native text for new Controls" and "More Controls as a template
Screen" below). Only Control Configuration and the Air Battle page draw
their Row text with the system font (`Step 1: …`, `Invert Vertical
Movement`). Adding a category or a Row to an artwork-labelled page
therefore needs new label artwork or a font-drawn label over a cloned
button model; adding a Row to a font-labelled page needs the page's
string source.

Open questions recorded before any new native Control is built:

- Answered 19 Nov: the Air/Land Battle speed and Invert Horizontal reach
  the stylus only. Measured on `s8` (land, camera yaw from the matrix at
  `0x141F8AEC`) and `s5` (air, reticle x at aim `+0xA4`), poking the save
  fields `0x00EF9790` (Land horizontal speed), `0x00EF9798` (Air speed) and
  the flags byte `0x00EF97AC` (0x41 as saved): a stylus drag turned 9°,
  33° and 80° at Land speed -1, 0 and 1 and moved the air reticle 30, 60
  and 120 pixels; Invert Horizontal reversed it in both. The mouse (the
  `0x00B12218` return) turned 14.76° and moved 12 pixels in every case, and
  the right stick was unchanged too. The battle reads the save fields
  themselves; the copies at the singleton's `+0x50..+0x64` read 0 in battle.
  Vertical inversion and Stop were not measured.
- Control Configuration (answered 19 Nov, driven through the page from a
  Practice Range Land Battle's Pause → Options): Step 1 picks how Pit
  moves, Circle Pad (Default) or the face buttons (the `????` is their
  glyphs); the face buttons lead to Step 2, how the reticle moves (Touch
  Screen (Default), +Control Pad, Circle Pad), and Step 3, a button layout
  with Reset, Try It! and OK. Its default for the face buttons: L Align
  Camera, R Attack, Circle Pad up Change View and left, right and down
  Attack, +Control Pad up Attack, left and right Select Powers, down Use
  Power, A B X Y Move. OK writes flags bit 0 (`0x00EF97AC`, 1 Circle Pad,
  0 face buttons) and applies it at once; poking the bit alone, even before
  a battle starts, changes nothing. With the face buttons chosen, a held
  Circle Pad moved Pit 0 units in 30 frames (9.9 before) and A, B and Y
  moved him 33: the Action Bindings' Move, Attack (L) and Change View (X)
  no longer do what they say.
- How the scheme is applied, and the port's hold on it (19 Nov):
  `0x002EEF10(controls)` reads the save's two choices and table and writes
  the live mapping into the controls object (the global `*0x0029BCAC` =
  `0x00D636E4`, holding `0x00EF9ED0`; mapping at `+0x3208..+0x35D8`). Both
  pages' OK call it, as does `0x002BD154` (File Select's, probably at a
  book's load; not seen headless, the load stalls). Pit takes the mapping
  at a battle's start or a resume from Pause, not mid-battle. Step 1 in
  Pause is Options page 3, its card the Pause cursor `+0x67C` (1 Circle
  Pad, 2 face buttons); Step 2 is page 4 and the button layout page 5. The
  main menu's Step 1 picks with `0x0021C86C(page, card)`, the card at page
  `+0x7B`. With the Circle Pad, Step 2 offers Touch Screen, +Control Pad
  and the face buttons for the reticle. The port (`circle_pad_only`) makes
  the save the Circle Pad's and the touch screen's at `0x002EEF10`'s
  entry, puts the Pause cursor back to 1 on page 3, picks card 0 at
  `0x0021C86C`, and in a battle has the input fold `0x00A4245C` call
  `0x002EEF10` once for a save still on another scheme (effective from the
  next resume). Checked in the 0.2.30 probe: two taps or Right on the face
  buttons card leave Step 1 on the Circle Pad in both pages; face buttons
  chosen with the hold off and OK pressed with it on gives a Circle Pad
  that moves Pit 9.9 units; a face buttons battle moves after a resume.
- The `????` scheme label: a glyph missing from the stand-in font or the
  game's own text. Darkshade's patch extends this page (Step 1 movement,
  Step 2 aim source) and ships RomFS assets for it.
- Where Pause's Options (battle UI code, `ctx+0x243C`, pages above) and
  the Scripted Menu Options get their Row strings.

Darkshade base patch (USA v172, Emulator package) against our hooks,
measured 13 Nov by comparing every `0x00xxxxxx` address in
`aim_mouse*.rs` with the patched `exefs/code.bin`: of 344 referenced
addresses in the code image, 3 differ, each one word turned into a branch
to the patch's code past the original image (code.bin 12,992,512 →
15,032,320 bytes, 25,198 bytes changed in the original range):

| Address | Our use | Original | Patched |
| --- | --- | --- | --- |
| `0x003901C4` | `PROJECTION`, projection getter entry | `e92d4030` push | `ea26947d` b |
| `0x0060D68C` | Pause `ToTop` place | `e2845a03` | `ea1c9f32` b |
| `0x006D2ED4` | `BEFORE_CAMERA_TURN`, the `bl 0x006D2ABC` | `ebfffef8` | `eb21b449` bl |

None of the five 60 FPS IPS words are patched. The package's RomFS is
`us/0.arc` and 56 stage `.zrc` files. Zakuro already lays a mod's
`code.bin`/`exheader.bin`/RomFS (`zakuro-core/src/mods.rs`), but refuses a
recompiled library built from the unmodified code when a mod changes it.

Native text for new Controls (13 Nov, `trace/s16.state`, Other): the
font hook `0x00A36FA4` runs once a frame there, with `r1` a UTF-16 string
in the heap message table (`0x1573F112`, "Adjust name, birth date, and Mii
settings.", followed by the other Options hints). So the top screen's hint
line is drawn from a string pointer a hook can substitute. Other's Row
labels (Profile, Subtitles, SpotPass) make no font call per frame: they
are model textures, drawn once or baked, so a new Row needs either their
text texture rebuilt or a font call of our own over a cloned Row model.

In battle (`trace/s8.state`, held 20 frames): D-pad Down shows the power
list over the power box (Health Recovery ×3); Select opens Pause; A, B, Y,
ZR and D-pad Up each showed the same purple burst at Pit, and holding L
gives a top screen identical to Y's, so in this control scheme they
attack as L does.

Held 45 frames from `user_rail` (air) and `s8` (land), against holding
nothing (two such runs are pixel-identical): X changes the view in both,
in the air the camera leaves Pit for the reticle, on land Pit spins his
weapon in a sparkle; R and ZL change the same number of pixels as each
other with Pit unchanged, only the enemy in the reticle moving, so no role
of their own shows. X is Change View: a tap on land turns the camera away
from Pit, and the game then shows its own hint "X: Change View" at the top
screen's bottom left (`trace/s8.state`, tap then 30 frames). The other
battle-control text in the heap is the scheme pages' "Moving Pit: ABXY" and
"Reticle control: ABXY". Battle pickups take effect at once, and the "item box" in
the table above is the power panel, so there is no held-item input.

Entering Other from Options by keys (`trace/s15.state`, Down, Down, A,
then 90 frames) makes no font call but the hint's: `0x00A36FA4` draws only
Options' hint (`r1` `0x1573E0F4`) and then Other's (`0x1573F112`). Other's
Row labels ("PROFILE", "SUBTITLES" in small-capital serif, "SpotPass") are
artwork in the Row models' textures, not text drawn by the font.

Other's models (`trace/s16.state`, with the top-screen menus on): Rows
`MenOptionOtherBt0001` (Profile; nodes NN_B0001, NN_B0002, NN_B0003_1,
NN_B0003, NN_B0004), `Bt0002` (Subtitles; NN_B0001..NN_B0005), `Bt0004`
(SpotPass; with its On/Off nodes), Back `Bt9999`, `Bt0000`, the window
`MenCmnWindowBt0000`, and a fourth Row `MenOptionOtherBt0003`
(NN_B0001..NN_B0005, the same shape as Subtitles) loaded in layer 15 but
given no place on the page. Writing 0 to a Subtitles node's shown flag
(model `0x14047040`, node table `+0x17C` → `0x140475E8`, flag at `+4`)
before a frame changes nothing drawn: the flags are set again before the
draw, so a Row without its label must choose nodes during the replay, as
`show_nodes` does.

## Recompiled boot continuation discovery

The unmodified USA executable (`code.bin` SHA-256
`2718aa2077a29560208c4e499288e5fecc4446d7adec95efb91ebf1b19eda679`)
executes ARM code at `0x00C511F8`. It continues the boot routine whose
discovered entry is `0x00C50DF8`; the recompiler previously had no code pieces
covering `0x00C511F8` through `0x00C51498` (exclusive). This is an entry into
existing game instructions, not a replacement implementation or game patch.

The first words are `03A06001 13A06000 EA00001A`: the continuation consumes
the incoming ARM flags, so its instruction mode and live context matter.
The observed interpreter history has even PCs in this range. A hint at
`00C511F8`, under title `0004000000030100` and text hash
`D6E028C6175A2B07`, lets discovery follow it. The hint is in the local
3dsrecomp fork's `hints.txt`; unrelated game revisions do not consume it.

Controlled acceptance, using a fixed console clock and fresh isolated
save-data directories, with hook-owning functions marked for interpretation
but hook bodies disabled in both runs:

- Boot, 300 frames: interpreter steps in the gap went from 11,490 to 0.
  Total outer interpreter steps went from 175,761 to 164,053.
- Remaining interpreted instructions outside translated pieces fell from
  11,864 to 374. Scheduler-budget fallback and intentional hooks remain;
  they are not missing translations.
- `s5` and `s8`, 60 frames each: interpreter counters were unchanged.
- All three runs had identical snapshots of 39 MiB of guest memory, the
  executable-text snapshot, and both screen images. Single-instruction
  interpreter callbacks from generated code remained zero.

The scripts, snapshots, interval-counter logs and equality hashes are under
`trace/boot-gap-cutover/`; `receipt.json` records the comparison. This does
not establish coverage of unvisited modes or remove the interpreter.

The separate linked Linux executable was rebuilt with the verified archive
and completed 300 headless frames. The loader reported 448,263 linked entry
points rather than the baseline's 448,238.

Zakuro's workspace now patches its existing `recomp3ds` dependency to the
local checkout as well as `recomp-abi`, so its Recompile button gets the
same hint as the CLI. Existing game libraries must be recompiled; replacing
the frontend executable alone does not rewrite them.

## Controls Row facts (from the removed PC Settings Row work)

The PC Settings Row and PC page built on these were removed from Zakuro;
only the game facts are kept here.

- A Row's look, chosen or not, is its model's animation (the players at
  `0x1403E4D8`–`0x1403E684` in `trace/s17.state` change frame, clip and
  callback with the Row outlined), not node visibility and not its
  materials. In `MenOptionControlBt0006`, `NN_B0001`/`NN_B0004` are the
  panel and border, `NN_B0002`/`NN_B0003` the Outline, `NN_B0005` the icon
  and label.
- Controls' and More Controls' Rows slide by a bone, not their model's
  world position (which stays 0, 0): `BL_B0000` in the skeleton at model
  `+0x22C` (world matrices at `+0x40`, 48 bytes each, x at `+0xC`). It is
  0 at rest and -35 while the Row is away, about 10.3 bottom-screen pixels
  a unit, and each Row slides in its own time.
- Text drawn through the font call takes its height from `ctx+0x24`
  (`[x scale, y scale, x, y]`), unscaled by the projection.
- L4 texture pixels are Morton-swizzled 8×8 tiles, low nibble first.
  `MenOptionOthersBt_Lang0001` (256×128, Profile, Subtitles, Hidden
  Options) and `MenOptionSousaBt_Lang00000` are in LZ11 `/us/menu/220.zrc`
  and `/us/menu/190.zrc`.
- A `TXOB` has `'TXOB'` at `+4`, name at `+0xC`, format at `+0x34`, height
  `+0x3C`, width `+0x40`, size `+0x44`, pixel address `+0x54`.

## More Controls as a template Screen (spike, 14 November)

Headless in `kiutrace` (interpreter only) from `trace/s23.state` (More
Controls) and `trace/s17.state` (Controls), with pokes and `code` patches
standing in for hooks; nothing in Zakuro was changed. Scripts and
screenshots are under ignored `build/spike-s23/`. Heap and VRAM addresses
are those of these states, not constants.

- Controls and More Controls are pages of one object (`0x1403E930`, vtable
  `0x00CF84DC`). Other is a different class (vtable `0x00CF2368` at the
  same address in `trace/s16.state`), and Control Configuration replaces
  the object when entered.
- Page switch: `0x003181C0(object, page, 0)`, page 1 More Controls, page 0
  Controls. Native A on More Controls reaches it with `lr 0x00318A40`.
  Called directly from Controls (`call 0x003181C0 0x1403E930 1 0`) it
  opens More Controls with the native slide-in, and B returns to Controls.
  Entry from Other was not tested.
- The four On/Off values are bytes on the object, 1 for On: `+0x119`
  Autofire, `+0x11A` Guide Arrow, `+0x11B` Aim Assist, `+0x11C` Shot
  Homing.
- Load: `0x00242654`–`0x00242690`, inside the setup function `0x0024235C`
  (its only reference is a pointer at `0x001BFE7C`), copies them from the
  save flag byte at `[0x00D636C4] + 0x16278 + 0x173` (`0x00EF97AB` here):
  bit 2 → `+0x119`, bit 4 → `+0x11A`, bit 5 → `+0x11B`, bit 6 → `+0x11C`.
  It runs when the object is created, before Controls is shown, not on
  entry to the page.
- Toggle: `0x00318C74(object, name record)` picks the byte by comparing
  the Row's name hash with four table entries (`0x003270EC` result
  `+0x14C`, `+0x178`, `+0x254`, `+0x280`), sets or clears it by
  `object+0x108`, and redraws through `0x00318DD0`. The write is at
  `0x00318D24`. The Row set is therefore inline code, not a data table.
- Redraw: `0x00318DD0` shows each Row's On or Off highlight from those
  bytes alone (`0x00318E90` onwards, through `0x0030296C`).
- Commit: leaf `0x003491A0(object)` packs the four bytes back into the
  flag byte. Callers are `0x002E1A60`, `0x00318828`, `0x00318860`,
  `0x003188E0` and `0x003189F0`. Toggling writes nothing to the save; the
  observed write (`pc 0x003491F8`, `lr 0x002E1A64`) came while backing
  out towards the Main Menu, after the B that leaves More Controls.
- Host values in and out: with `+0x119..+0x11C` poked on Controls and
  `0x003491A0` patched to `bx lr`, native A opened More Controls showing
  the poked values, Left on Guide Arrow flipped `+0x11A` with the native
  animation, and after backing out to the Main Menu the flag byte was
  unchanged (`0x52`).
- Hint: the one font call a frame (`0x00A36FA4`, then `0x005CB24C`) draws
  the hint from the heap message table (`0x1573E3B4`, "Display a green
  arrow directing you where to go."). Overwriting that string in place
  drew the replacement in the native font and position.
- Labels: no font call draws them. The Row labels, ON/OFF and Try It! are
  the 256×256 L4 atlas `MenOptionSousaBt_Lang00000` (`TXOB` at
  `0x15BA14C8`, pixels in VRAM at `0x1F416200`, `0x8000` bytes).
  Overwriting those pixels changed the drawn labels on the next frame, so
  the atlas can be repainted live.
- The top screen's preview (Pit, the `?` bubble, the arrows) follows the
  Rows' values; a page reusing this Screen for other settings needs that
  layer hidden or replaced.

## Lifted-code discrepancy probes (14 November)

These are experiments under ignored `build/re-next/`, not production
C Overrides or additions to the recovered-source count. The baseline is
`build/continue/lift-frame/`; verification counts belong in
`docs/decomp/LEDGER.md`.

- `0x00B66020`: the original instruction at `0x00B6610C` reads
  `[r0 + 0x88]` only after the preceding branch has found `r0 == 0`.
  Captured reference calls avoid that path. Ghidra also includes synthetic
  `COPY` operations from memory location `0x88` to itself at the function's
  exits. `LiftOverrides.statement`/`assign` emits these as a real read and
  write, causing an otherwise successful candidate to leave captured memory
  at `0x88`. Removing identical-address, identical-size literal-memory
  self-copies fixes those captured calls. It does not justify suppressing
  the original conditional read or mapping the null page.
- `0x00B421A4`: `bl 0x00B40F28` at `0x00B42204` produces `s0`, stored
  by `vstr s0, [r4, #0x14]` at `0x00B42208`. Ghidra instead treats `s0`
  as an incoming value surviving the call; the lift stores `lift_in_vfp[0]`.
  Refreshing that one local from `ctx->vfp[0]` after the call fixes the
  captured discrepancy. Call arguments and targets agreeing is insufficient
  to establish a correct call contract: results also need checking.
- `0x00A3F3F0`: `0x00A3F194` writes six words through the pointer to the
  first local bounds at entry `sp - 0x38`. The lift initializes locals from
  that memory before the call, then fails to refresh five of the words.
  At the first loop header, Ghidra's `MULTIEQUAL` joins stack values and
  register/unique values in the same `HighVariable`.
  `LiftOverrides.phiCopies` skips equal-`HighVariable` copies even when
  memory must supply the value. Refreshing those five locals after the
  first call fixes the captured calls. Recursive and child-iteration paths
  still need separate evidence before a general repair is accepted.
- `0x00637F5C`: parameters at the object behind `+0x78`, fields `+0x24`,
  `+0x28` and `+0x2C`, can contain quiet NaN `0x7FC00000` in actual calls.
  Self-comparisons at `0x00638274`, `0x006382A0` and `0x006382CC` detect
  unordered values and preserve fallback values. High p-code drops those
  self-comparisons. Restoring the three checks in the candidate fixes its
  captured discrepancies. Setting `DecompileOptions.NanIgnoreEnum.None`
  does **not** fix this case: the independently lifted candidate still has
  no `isnan` in this function and differs on the same three captures.
  The installed Ghidra 12.2 source explains a separate loss in
  `src/decompile/cpp/ruleaction.cc:2380`,
  `RuleTrivialArith::applyOp`: identical operands make `FLOAT_EQUAL` and
  `FLOAT_LESSEQUAL` constant true, and `FLOAT_NOTEQUAL`/`FLOAT_LESS`
  constant false, without consulting the NaN options. The ARM
  `data/languages/ARMneon.sinc:1619` comparison macro supplies Z from
  `op1 f== op2`, while NaN detection separately supplies V. Branches testing
  Z after a self-comparison can therefore lose the NaN case even with
  `NanIgnoreEnum.None`. This is upstream of the lifter and independent of
  C compiler optimization; the explicit-check probe is not a general fix.

Evidence: `build/re-next/pcode.txt`, per-function `.ref.c`/`.lift.c`,
`DecompileOptions.javap.txt`, `probes/`, `nan-preserved/`, replay logs and
`receipt.json`.
The focused capture directories retain copies of the original capture
headers and link to their original page stores; original captures are
unchanged. Probes establish causes on captured inputs, not general semantic
equivalence or valid high-level types.

## Lifter repairs and call-contract continuation (14 November)

`ghidra/LiftOverrides.java` now suppresses `COPY` when both operands name
the same memory address and size. Its phi transfers reload memory inputs
even when the destination shares their `HighVariable`. On the existing
capture sets this fixes `0x00B66020`, `0x00A3F3F0`, `0x0049B79C` and
`0x003B5B94`; the latter two are additional observed improvements from the
general repair, not hand-recovered functions.

Five further saved-state scenarios (`exo-tank`, `s5`, `s6`, `s7`, `s9`),
after 20 neutral frames then A and circle-pad input, supplied 21 new calls
of `0x00B66020` and 10 of `0x00A3F3F0`. Reference-self replay and the repaired
lift match all 31. Every captured bounds call still had zero parent and
child links, so these captures alone do not exercise the hierarchy paths.

Eight explicit hierarchy fixtures extend one captured machine state with
a separate scratch page. They cover a leaf, one child, two child orders,
an equal child, a parent, a parent with children and two parent links.
For the native sphere kind, `0x00A3F194` uses position at `+0..+8`, radius
at `+0xC` and kind bits at `+0x20`; a zero secondary length at `+0x5C`
disables the additional swept volume. The hierarchy links used by
`0x00A3F3F0` are parent at `+0x80`, first child at `+0x84`, and next sibling
at `+0x8C`. An independent mathematical union of the sphere bounds agrees
with the reference in all eight fixtures. The storage repair removes all
five ordinary baseline discrepancies; seven fixtures still differ only in
`r0`. These are constructed fixtures, not observed game-owned hierarchies,
and the remaining machine-return differences are unresolved.

NaN handling now sets `NanIgnoreEnum.None` **and** disables Ghidra's
`trivialarith` rule through the native `togglerule` option. The configuration
is reapplied in `SemanticDecompiler.initializeProcess`, including when the
native process is restarted; initialization failure aborts the lift.
Disabling the rule preserves the original float self-comparisons directly,
rather than adding address-specific NaN guards after lifting. The six
captured calls of `0x00637F5C` match with this configuration. The rule also
simplifies integer expressions, so full-batch measurements accompany this
change in the ledger.

The float-result experiment in
`build/repairs-20261114/return-scripts/LiftReturnProbe.java` gives two callees
custom `s0` return storage: `0x00B40F28` and `0x00B49860`. Both write `s0`
in the generated reference; `scripts/contracts.py` also identifies their
callers reading `s0`. Parameter inference is committed before adding return
storage, and an outer transaction rolls every signature edit back. The
eight targets are these callees and six lifted callers. This removes the
observed memory discrepancies in `0x00B421A4`, `0x00BAFB70`, `0x00BB04F0`,
`0x00BB08C8` and `0x00BB1070`. Only `0x00B421A4` becomes fully matching;
the other four now differ only in `r0`, and the two callees regress from
fully matching to `r0`-only. `0x00BB0C9C` remains matching. A single typed
float return does not preserve the independently observable core-register
effects. The prototype edits are experimental and are not installed in the
lifter or Ghidra database.

Evidence is under `build/repairs-20261114/`: generated variants,
`storage-summary.json`, replay receipts, `capture-topology.json`,
`make_bounds_fixtures.py`, `bounds-fixtures/`, `return-scripts/` and
`returns/`. New actual-entry captures are in
`trace/re-storage-20261114-a/`; previous captures and the game dump are
unchanged.

## Simultaneous core and float call results (14 November continuation)

The result contracts for `0x00B40F28` and `0x00B49860` now explicitly carry
both `r0` and `s0`. `ghidra/kiu-call-results.txt` binds these contracts to
the original executable SHA-256; the lifter rejects other images. Words
are listed least significant first (`r0,s0`), reversed for Ghidra's
high-to-low compound storage. The temporary unsigned 64-bit return is a
data-flow container, not a recovered C type or a claim about the game's
declared signature. Calls read the actual register storage, and returns
split the result back into both register banks. Treating an eight-byte
result as the ordinary `r0,r1` pair would still be incorrect.

- `0x00B40F28` counts enabled bits 0 through 3 at object `+4`. Without bit
  3 it leaves the count in `r0` and loads the default float into `s0`.
  With bit 3, it leaves the selected packed-field address in `r0` and loads
  that word into `s0`. Both outcomes are observable in the reference.
- `0x00B49860` leaves the list-anchor address (object `+0x14`) in `r0`.
  Starting from the default float, it multiplies the words at payload
  `+0x10`, following the node pointers from object `+0x18`; `s0` carries
  the product. This establishes the accesses and arithmetic, not ownership
  or a game-level meaning for the factor.

All eight functions in the previous float-return experiment now match all
59 recorded entries. The five callers whose memory discrepancies are
removed are `0x00B421A4`, `0x00BAFB70`, `0x00BB04F0`, `0x00BB08C8` and
`0x00BB1070`; both callees retain the correct `r0` as well as `s0`.
The negative-control caller `0x00BB0C9C` remains matching.

Replay previously did not compare `s0` at function exit: it is ordinarily
caller-saved, but is also a result register for these callees. The old
"same" results for their own entries therefore did not prove their float
outputs. `REPLAY_VFP_RESULTS=1`, together with `REPLAY_ONLY`, now checks
those known result contracts. The original lift differs in `s0` on nine
of the seventeen direct callee entries; the paired-result lift matches
all seventeen. The default comparison of other functions is unchanged.

Forty constructed result fixtures add coverage beyond the recorded calls:
32 packed-mask cases (all low-nibble combinations, with and without upper
bits) and eight list-product cases. They cover empty and multi-node lists,
raw NaN payload loads, signed zero, infinity and signed underflow. A
constant-result oracle independently specifies both register words and
matches the reference. The paired-result lift matches all forty. A mutant
that flips one bit of `s0` passes with float comparison disabled and fails
all forty cases with it enabled. These are deliberate fixtures, not
observations of game-owned object graphs.

### Signature lifetime and generation failures

Ghidra's `GhidraScript.executeNormal` starts a transaction through
`FlatProgramAPI.start`. `DomainObject.endTransaction` documents that nested
aborts only take effect when the final enclosing transaction ends. Thus
earlier references to per-function rollback describe the intended scope,
not the actual lifetime: inferred signatures can influence later lifts
until the script exits. The maintained result contracts are provisional
and roll back at script exit; general target-order independence is not
claimed.

The experiment in `build/multi-return-20261114/LiftOverrides.isolated.java.txt`
ends Ghidra's automatic transaction and reapplies contracts in separate,
truly reversible per-function transactions. The eight-function cohort is
stable in reversed order, but the full batch loses useful inferred
argument information in other functions. For example, `0x008DBBB8`'s
second call to `0x0018C7BC` loses its third argument; the latter's own code
reads `r0`, `r1` and `r2`. Other changed verdicts and witnesses are retained
in `isolation-real-changes.json` and `isolation-random-changes.json`.
This broader isolation change is **not** enabled in the maintained lifter.
Making the dependency contracts explicit is required before adopting it.

Headless Ghidra can also exit zero after a script exception. `scripts/lift.py`
now generates into a fresh temporary directory, retains `ghidra.log`,
checks that the complete ordered target report exists, and only then
replaces the previous candidate/report. Wrong hashes, invalid registers,
duplicate result registers and partial/failed generation cannot silently
compile a stale candidate. Three unit tests and actual contract-rejection
checks cover these paths.

The final generated batch and all receipts are under
`build/multi-return-20261114/`; `final-summary.json` compares the final
maintained configuration with the previous candidate. The forty fixture
headers and their separate pages are under `fixtures-v2/`. Experimental
isolation artifacts are retained separately. No generated candidate has
been installed in `overrides/` or the Windows game-library cache.

## Maintained scalar storage routines (14 November continuation)

`overrides/scalar.c` replaces `0x00B40F28` and `0x00B49860` with hand-written
C. The names describe observed storage only: packed mask, list anchor,
wrapper payload, next pointer and factor. No ownership or gameplay meaning
has been assigned to these fields.

The packed routine's mask is at object `+4`. Its low four bits select packed
words; bit 3 selects the last word as the scalar. The observable result pair
is a count/default without that bit, or the packed address/raw scalar bits
with it. Loading a NaN payload must preserve the original bits. The original
default literal is at `0x00B40F98`. Generated block charges are 9 on entry,
four visits each to blocks of 5 and 11, then 3 more on the default path:
73 instructions with the scalar, 76 without it.

The product routine returns object `+0x14` in `r0`, reads the first wrapper
from object `+0x18` and starts with the literal at `0x00B49890`. At each
wrapper, payload `+0` is compared with the anchor's first word; equality
terminates, independently of the next pointer. Otherwise factor bits are
loaded from payload `+0x10`, wrapper `+4` supplies the next pointer, and the
factor multiplies the accumulator. Each multiplication uses the runtime's
FPSCR-aware output flushing. The generated charge is 9 plus 8 per factor.

A deliberate list test with 20,000 valid graphs, lengths 0 through 32 and
arbitrary factor bits exposed 52 NaN-result discrepancies in the first C
version. Core outputs and charges already matched. GCC's generated reference
selected the factor's NaN payload when both operands were NaNs; compiling
the direct nested C expression selected the accumulator's payload instead.
The maintained implementation explicitly quiets and preserves a NaN factor
before ordinary multiplication. The same 20,000 cases then match, including
raw `s0` and charges. This is reference-library fidelity, not independent
proof of 3DS hardware NaN/default-NaN behavior. Directed regression checks
pin two distinct NaN payloads, the packed results and both charge formulas
under GCC `-O0` and `-O2`.

The full library's entry table and direct calls to these routines point to
the C Overrides. It was generated with all twelve maintained overrides;
525 objects were reused only for byte-identical source files, and four
changed files were recompiled. The ABI header is identical; the function
header adds only the two override declarations. Build and verification
artifacts are under `build/scalar-20261114/`. This Linux library is separate
from the Windows frontend and has not been installed in its game cache.

## Windows Recompile with pinned C Overrides (14 November continuation)

Anchors: `tools/3dsrecomp/src/overrides.rs` (`Pinned`),
`tools/3dsrecomp/src/build.rs` (`pinned_files`, duplicate checks),
`tools/3dsrecomp/src/compile.rs` (`common_folder`), and
`crates/zakuro/src/recompile.rs` (`PINNED`). The pinning key is
the FNV-1a hash of the executable's text after mods and updates, the same
key as the bundled hints; for this dump it is `D6E028C6175A2B07`.
Override seeds change function partitioning across nearly every generated
unit, so a same-tree no-override reference needs a full rebuild; it cannot be
derived by recompiling a few files. Results are in `docs/decomp/LEDGER.md`.

## Armory — Powers freeze: Windows delivery check (16 November)

`aim_mouse/menu_top.rs::font` filters the shared bottom text pass to the
native rectangle of the text-bearing Control being replayed. Scissoring
alone still allocates the other Controls' glyphs in the game's shared draw
buffer. The existing regression is
`armory_text_replay_omits_glyphs_owned_by_other_controls`.

The source already contained that filter, but `dist/zakuro-kiu-mouse.exe`
was an older build (`2e60f1b1deb70fa6ee61a25956305d78b3d86705305cad691488ff97306cf3d5`).
Rebuilt the Windows release and replaced it; the build and distributed file
both hash to `6402f46598d661722755672b2ebbf92f8fff105ba7340dfb5944e3eb0a6594b7`.
All 32 `aim_mouse::menu_top::tests` pass.

Validation uses a copy of the current Windows save files, booted afresh,
then File Select → Solo Hub → Gear → Powers. Its full board includes Sky
Jump, Idol Transformation, Land Mine and Quick Charge, matching the user's
reported inventory. The Windows shared-device GPU replay at 3x responds to
mapped touches and returns to Gear with Back after 112 frames. The sparse
historical Powers state was checked separately; it is insufficient by itself
for this report. Artifacts and scripts are in `build/powers-delivery/`.

While the earlier Zakuro instance was open, launching the replaced `dist`
path failed with `0xC0000139`; the identical build and fresh-copy paths
started successfully. After the user closed the earlier instance, the
distributed path completed its five-frame Windows headless startup check.
Interactive physical mouse acceptance remains unverified.

## Armory — Powers: independent top artwork layers

`screen_transform.rs::POWERS_TOP` appends five mesh-owned Placements to
Screen 18, preserving Controls 0–5 and Details card 6. `MenEquipUp0200`
owns Paper background (7); `MenEquipUp0202` owns Power set board (8).
`MenEquipUp0210` nodes `NN_U0000`, `ML_B0000`, and `Wbase` own Power
preview (9), Power shape (10), and Description background (11), respectively.
The preview's grid and icon frame share `NN_U0000`. The remaining
`MenEquipUp0211` decoration and text stay in Details card.

`menu_top.rs::text` skips `TOP_TEXT` (`00727F10` → `00727F14`) for
row-declared mesh-only top layers. Calling the text renderer and filtering
its glyph calls afterward was insufficient when adding Powers' artwork
replays: the Screen stopped responding after the first drawing interval.
The earlier glyph guard remains for calls that reach it outside this pass.
The new regression exercises the early skip for Gear and Powers artwork.
Isolation, geometry/visibility edits, and Back replays are under
`build/powers-top-layers/`; validation uses the current full save's Powers
board rather than only the sparse historical state.

## The game's menu identity (MENUID)

Menus are named by an enum whose names are in `.rodata` from `0x009511A8`
in enum order: `MENUID_START` 0, `STAGE_SELECT` 1, `SELFILE` 2, `HOME` 3,
`TOP` 4, `BET_HEART` 5, … up to `MENUID_MAX`. `0x00950E7C(hash*, u8* out)`
turns a name hash into that number (each name's hash cached in a static
block at `[0x00950E7C's pool +0x318] + 0xB4…`); no direct ARM or Thumb
call to it was found, so it is reached indirectly.

Each live menu is a task-tree node carrying its name inline at `+0x20`
(`MENUID_HOME`, `MENUID_STAGE_SELECT`); node `+4` is the node itself,
`+8` its child, the Screen's controller (vtable `0x00CFC3D0` for the hub,
`0x00CF243C` for Chapter Select), whose own `+8` points back at the named
node. From `/mnt/c/.../states/0004000000030100/2.state` (Chapter Select,
going back to the hub on load): the hub's named node and controller are
built during the 19th frame (after `run 18`), and Chapter Select's are
unlinked then. The name is not unique to one node: every object the menu
owns (constructor `0x00417734`, vtable `0x00CA046C`, 13 for the hub, 33 for
Chapter Select) carries the same `MENUID_…` name at `+0x1C4`.

### Current MENUID: static path

```
u8 MENUID = [[[0x00E6F98C] + 0xC] + 0x72C]
```

- `0x00E6F928` is a static singleton (vtable `0x00CEEA5C`; its address is
  cached at `0x00D63608`, filled lazily by `0x00BB79F4`). Its `+0x64`
  (`0x00E6F98C`) points at the current menu's 0x10-byte holder, vtable
  `0x00CFC5C0`. `+0x60` was 0 in both menus. `+0x68` bit 0 is "current menu
  is ending".
- Holder `+0xC` points at the menu body (0x848 bytes, vtable `0x00CFC314`).
  Holder `+8` (u8) is 1 once the menu has ended and is waiting to be
  replaced.
- Body `+0x72C` (u8) is the MENUID. Body `+0x724` points at that menu's
  static record, `0x00EFD814 + MENUID * 0x44` (`0x00EFD858` Chapter Select,
  `0x00EFD8E0` hub), so it identifies the menu as well. Body `+0x728` is
  `0x00EFD528`.
- The bytes after `+0x72C` are not cleared (`+0x72D…` held stale text);
  read one byte. Check for a null slot and the two vtables before trusting
  the value: other game modes were not examined.

Setters, all in one frame:

1. `0x0071819C(0x00E6F928, 0, 0)` destroys the old holder and zeroes the
   slot (`0x007181F0`), and clears `+0x68` bit 0.
2. A per-menu factory calls `0x00A3B9AC(holder, MENUID, record,
   0x00EFD528)` with its MENUID as a constant: `0x00814EE8` for the hub
   (3), `0x008330AC` for Chapter Select (1); 57 call sites in all, reached
   through pointers (`0x00CF9614` holds the hub's). `0x00A3B9AC` builds the
   body (`0x00A377A4`) and calls `0x00A36260(body, MENUID)`, which stores
   the byte at `0x00A36298` and then builds the menu's objects through
   `0x002EC69C` → `0x003AF170` → `0x003F601C` → `0x00417734`.
3. `0x0071819C(0x00E6F928, 0, holder)` stores the new holder in the slot.

The slot is null only inside that frame; at every frame boundary it was
valid. Holder and body were reallocated at the same heap addresses
(`0x149A6930`, `0x14012688`) in both directions of this run; do not rely
on that.

Timing, both directions (Chapter Select → hub from the state; hub →
Chapter Select with A held three frames on Move Out at frame 150):

| | Chapter Select → hub | hub → Chapter Select |
|---|---|---|
| Holder `+8` = 1 and singleton `+0x68` bit 0 set (`0x00A3B984`, `0x00B6CAA4`) | frame 2 | frame 157 |
| MENUID changes (steps 1–3) | frame 19 | frame 165 |
| Both screens black | after frames 19–21 | after frames 165–167 |
| New menu first drawn | frame 22 | frame 168 |

So the MENUID changes in the first black frame, three frames before the
new menu's first drawn frame, and the old menu's fade-out is still
reported as the old MENUID. The "ending" byte leads the change by the
length of the fade (8 frames after a press, 17 here from the state). No
"next MENUID" value was found ahead of the change: the next menu exists
only as the constant in the factory that gets called. `0x00E11E0C`
(`[ptr, 0x100, ptr, 0x101, …]`) is a table of the menu's objects with
their handles, not a menu field. Words going 1 → 3 early (`0x15B1D88C` at
frame 8) sit in repeating 0x14-byte records and are not a menu field.

Used first for the hub's heart count: `menu_top::current_menu` reads this
path (checking both vtables), and `font` holds the native count back
whenever it is `MENUID_HOME` with menus on top, so the count no longer
waits on the hub's controller being recognised. Harness: 1 on Chapter
Select (frame 1 of `2.state`), 3 on the hub (frame 30).

MENUID for every scripted Screen (enum 0–58, read from `0x00950E7C`'s
table): FileSelect 2, ChapterSelect 1, MainMenu 4, Options 17, Other 22,
Controls 19, ReticleCamera 25, Subtitles 23, Volume 21, Profile 39,
ProfileYesNo 45, SoloHub 3, ArmsAltar 11, ArmoryGear/Powers 14, Redeem 12,
Convert 37, Fuse 26, ArmoryPowers 18, Weapons 43, Categories 42,
PracticeRange 10, both Idol Tosses 44, TreasureHunt 13, Vault 16 (its AR
and Records pages too). The SpotPass dialog shows under Other's 22, the
idol coin dialog under 44. No scripted Screen: BET_HEART 5 (Cauldron),
OPTION_CONTROL_BUTTON 20, Music Gallery 38, Vault Idols/Puzzle/Gift 46–48.
`menu_top::scene` now keeps the input-confirmed Screen only while it
agrees with the MENUID, then takes the MENUID's Screen, and guesses from
model names only for MENUIDs with no single Screen. Harness `menuid`
prints both.

## Idol Toss clipping and egg dragging (16 November)

Restyle Launcher, Fountain and Egg queue on Idol Toss: meshes cut at their
Placement rectangles → complete meshes drawn through the shown screen.
Rebind Egg queue on Idol Toss: no pointer input → held Synthetic Touch from
the queue into the bowl. Acceptance: the Windows GPU replay from slot 4
moves the first egg into the bowl, changes NEW to 86.4%, and keeps that egg
inside the bowl after moving and resizing Launcher.

`menu_top.rs` treated the node-isolated Idol groups as cropped Boards: both
the viewport/projection and the scissor could cut a moving bowl at its hit
box. Back, Launcher, Fountain and Eggs now use the same full-screen placed
projection and scissor as other isolated meshes. Counters and Dialog keep
their text cuts. Eggs remains behind Launcher in draw order, but now takes
pointer input; a held drag transfers from its Placement to Launcher's when
it enters that Control, without releasing the native touch. Fountain still
takes no touches.

A loaded egg remains `MenColVisionBt0002`, the same model as each queued
egg, so assigning that entire model name to the queue misplaced the loaded
egg even when the touch succeeded. Its root bone's world matrix (model
`+0x22C`, skeleton `+0x40`, translation `+0xC`) moves from x=13.5 in the
queue to about x=0.79 in the bowl. The native orthographic camera maps x to
`160 + 10*x`; the queue's source Placement starts at x=262. Each egg draws
only with the group containing its root: Eggs while on that rail, Launcher
once it leaves. The existing node visibility and native depth ordering
remain authoritative.

Artifacts: `build/toss-fix/`. The new drag and scissor regressions were
observed failing before the fix. The final Idol checks pass 9/9; the mouse
suite passes 97/97 with the pre-existing
`placed_scripted_text_omits_the_same_colour_offset_copy` failure excluded
(its test and `font` implementation are unchanged from HEAD). Linux and
Windows interpreter GPU replays and the Windows installed-library GPU
replay all consume the first queue egg and show it inside the bowl; image
assertions detect zero pale egg pixels in the vacated slot and over 1,100
inside the bowl. Windows uses the frontend's shared-device GPU path. The
edited Launcher replay also keeps its egg aligned. Physical mouse input
has not been exercised.

The Windows release was rebuilt and replaced at
`dist/zakuro-kiu-mouse.exe`; build and distribution SHA-256 both equal
`74b95da2f84ebbb78f80b8c27788f0042eb45e78846b3e12540ed38f02b467d0`.
The distributed executable completed its five-frame Windows startup smoke.
The release link succeeded outside the sandbox after two sandboxed build
processes were terminated; no delivery blocker remains.

## Hide Idol Toss Launcher and Fountain during the reveal (16 November)

Hide Launcher and Fountain on Idol Toss: both cover the revealed idol →
both disappear during the top-screen launch/reveal and return to their
existing placements when egg tossing resumes. Acceptance: saved-state
replays show both parts while tossing, neither over the revealed idol,
and both again after dismissing the reveal.

The native state byte at the Idol Toss object `+0x430` drives the timing:
0 idle, 1 pending launch, 2 launch, 3 waiting for the launch animation,
4 idol reveal, 5 returning to tossing. `0x0021546C` advances the launch
states; `0x002155F4` sets phase 5 and restores the tossing input flag.
Do not use `+0x35 & 0x40`: that means a newly obtained idol, so it would
miss duplicate idols. Slot 4's object is `0x140439D0`; a native pull and
release followed by 120 frames reaches phase 4, and A followed by 80
frames returns to phase 0.

The Idol layout strategy samples phases 2–4 once in `menu_top::begin`.
`MenuTop::available` excludes only Launcher and Fountain from drawing and
hit-testing during those phases. Saved editor geometry and visibility
are preserved. Other groups and Screens keep their existing behavior.
Artifacts and validation logs are in `build/toss-reveal/`.

Validation: 10 targeted Idol checks pass. The Windows shared-device GPU
replay with the installed game library passes idle → reveal → returned:
orange/yellow pixels in the bowl and pedestal probe rectangles are
[1480, 428] → [0, 0] → [1604, 428]. The distributed Windows executable
completed its five-frame startup check. Windows release and distribution
SHA-256 both equal
`6f6a794042b0727ecdf4de58ebc17410c4efabe39bb2d205944d1d8162419534`.
Physical mouse testing remains unperformed. The Linux interpreter replay
captured the corrected reveal but was terminated before its return capture;
Windows completed all three captures.

## Idol Toss pop-ins on entering, leaving and the play-coin Dialog (20 November)

Idol Toss is one Screen to the game, from the Solo Hub and from the Vault
alike. For either one's object the class runs 0x001B4D4C alone on its first
frame, then 0x001B4D58 and 0x001B4D4C every frame (harness `calls`, also
during a held egg drag), and only 0x001B4D58 while leaving. So the
registry's rows 22 (0x001B4D58) and 25 (0x001B4D4C) never told the two
apart: the Screen was 25 every steady frame, from the hub too, and the
saved edits are all under 25. It became 22 on the leaving frames and
under the play-coin Dialog (hard-coded 22), and those frames drew 22's
default Placements: everything jumped while the scene faded out, and
jumped when the Dialog opened and again when buying closed it.
`menu_top::screen_of_row` now draws and saves row 22 as 25, and the
Dialog uses 25.

Entering from the Vault, the first frame the Idol Toss models loaded drew
the native fallback layout (`scripted_len() + 6`: a Details card a third of
the screen wide and the whole bottom screen as one Board), because MENUID 44
names both rows and the model scan kept Idol Toss only when the last input
was already one. `MenColVision` models now draw Screen 25.

The frame after that still drew the Fountain where the bottom screen had
it. A new scene's bottom camera has no projection (camera `+8` bit 2
clear, flags 0x01; 0x45 once 0x003901C4 builds it, 0x77 after a draw)
until the first draw through it, and the replays run before the game's
own bottom pass: the first replay drew unplaced. Building the projection
first (calling 0x003901C4) was not enough; the rest of the camera is also
set by that first draw. Bottom parts whose camera is unset now wait for
the next frame (`menu_top::draw_part`). This fires on exactly one frame per
scene change (entering Idol Toss, Chapter Select, the Vault) and on none
while a Screen is steady.

Under the user's saved layout a dragged egg follows the pointer within
about 3 px across the hand-off from Egg queue to Launcher. Under the
default layout it shrinks from scale 1 to 0.68 and drops about 27 px where
it leaves the queue (native x 262), because the default Egg queue keeps
its native size down the right edge while the Launcher is at 0.68. The
native touch ring and sparkle trail are not drawn on the top screen. The
green leaves dropping off a dragged egg and the large Back arrow sweeping
the top screen while leaving are the game's own.

Harness: `kiutrace` `override <screen> <part> x y w h` loads a saved
`menu-layout.toml` layer, as the frontend does. `build/toss-popins/` holds
the replays (`enter`, `exit`, `coin`, `buy`, `drag`), the user's layout as
`override` lines, and 0.2.32 states reached from slot 2: the Vault, Idol
Toss, the Dialog, an egg ready to drag, a loaded egg and the Solo Hub.
0.2.30 states no longer load. The leaving, Dialog and buying replays keep
Counters at (0, 4) and the Launcher at y 102 throughout; entering goes
from 24 straight to 25.
Windows release rebuilt; `dist/zakuro-kiu-mouse.exe` and the Downloads
copy both have SHA-256
`36badbf624a9b186368cd6392dde5536bfd8a8a5185438d7b648610de05d84cc`, and
the distributed executable completed its five-frame Windows startup check.
Physical mouse testing remains unperformed.

Default Egg queue at the bowl's scale (20 November): Launcher, Fountain and
Egg queue now share `idol_toss` by default, bottom-anchored with the queue
flush right (at 16:9 the bowl's top moves from y 62 to 104, the queue
[387.2, 76.8, 39.4, 163.2]). Replaying `build/toss-popins/bought2.state`
with no saved layout, a dragged egg stays within 1.5 px of its previous
offset from the pointer every frame and keeps its size, where it used to
shrink from scale 1 to 0.68 and drop 27 px leaving the queue. Saved edits
still override the default.
Windows release rebuilt with it; `dist/zakuro-kiu-mouse.exe` and the
Downloads copy have SHA-256
`c71d3a0dadf07f79f6538d2f19b3ba7b4c6e14517bcf13631a8644e2b04f2013` and
completed the five-frame Windows startup check.

## Midstage Loading layers (22 November)

The loading between an air and a land battle (user's slot 3, Chapter 1's
mid-stage, loading from frame ~130 to ~435) is no Scripted Menu: no MENUID,
no scene models. Its pictures come from the dialogs' 2D context
(0x14A1340C, the Retreat Dialog's) with bit 26 of +0x270 set:

- Top pass 0x007255AC keeps that bit in r7 and tests it at 0x00725784.
  With it set: the tip bar, 0x0092EDA8 at 0x007257D0 (s0..s3 = 0, 181,
  400, 59) flushed at 0x00725814 (returns to 0x00725818); then 0x00724AE0,
  whose flush returns to 0x00725828 like the battery's every frame (the
  battery quad, 382..400 × 0..13, is already dropped there): one turning
  64-pixel quad, middle about (196.6, 105.3).
- The tip's words: the 3D renderer, camera call 0x0090A5B4 at 0x00725970
  with the top projection on the stack in r1 (rows -1/120 and -1/200),
  then 0x00909ACC(r5 + 0x19C) once per eye.
- Midstage Progress: the bottom pass 0x00722A50 draws one text, 0x00909ACC
  at 0x00722DAC (r0 = ctx + 0x204), through the same call sites the
  Retreat Dialog's words use; it draws no box (0x00722AF4/0x00722B0C are
  not reached). With Pause on the top screen these replayed at
  DIALOG_TEXT (0x007258C0) through the dialog's panel rule, so before this
  change they covered the orb and the tip bar.
- The bottom pass draws the words for three frames after the top's last
  loading pass; the next top passes have r7 clear.

The Midstage Loading is a Screen of its own with `LoadingRules` (Tip bar,
Tip, Loading orb, Midstage Progress); see screen-transform.md. Harness
`layers`, `layer`, `visible` reach it while it is the Screen last drawn.
Checked from slot 3 (`pausetop narrtop menutop`): default at 5:3 and 16:9
keeps the parts apart; moving the orb and Midstage Progress and hiding the
bar moves and hides each alone; entering and leaving frame by frame show no
part in another place; a Retreat Dialog later in that battle still draws in
its own panel.
