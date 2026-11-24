# 8. Node Layers Are Drawn Whole

A Board's bottom node layer was cut at its native box: a hand-measured rectangle that also placed the layer and took the pointer. Measured on one page of a Screen, the box cut every page whose parts are bigger or sit elsewhere (Chapter Results: a weapon's stats card is taller than a Power's bar), and art that reaches past a button's nominal box (Take's frame, the heart chip's rounded end) lost its edge. The cut was never what kept layers apart: a node layer already replays only the mesh nodes it owns, and its words are the strings anchored in its box (`font`), drawn or skipped whole.

We decided that a node layer is drawn whole. Its box is only its frame (where the Placement maps from, and where the pointer takes it), never a cut. The only cut left is the game's own scissor, such as a scrolling list's window, mapped through the same Placement. A list the game scrolls past its window and hides with a mask mesh in another layer is the one declared exception: its box is its window and it stays cut (`LIST_WINDOWS`). Chapter Results, Fuse Weapons and Idols are drawn so (`UNCUT_BOARDS` in `crates/zakuro-core/src/aim_mouse/menu_top.rs`); the other Boards keep their cuts until each is moved over and its pages are checked. On Fuse, drawing whole showed the scroll bars' arrows, which the cut had hidden, and showed each weapon list's overflow until both lists were declared windows. On Idols, it showed the white panel over the Title bar's band, which the game draws over it (`UNDER_LAYERS` stacks the panel under), and the Zoom bar's knob, which the panel's cut had hidden in the wrong layer.

## Considered options

- **Measure each layer's box from its geometry.** Each mesh's CGFX shape holds an oriented box, and the skeleton's world matrices with the bottom camera place it on the native screen (`layer_rects`, kiutrace `rects`). Rejected as the box: it counts transparent texture margins (Convert to Hearts reuses Take's quad), meshes made invisible by their material (the chip's flying heart), and off-screen placeholders (Message's text node sits at x≈5070). It stays a harness aid for writing boxes.
- **Keep cutting, with bigger boxes per page.** Rejected: every new page or animation is another measurement, which is the flaw.
- **Read drawn bounds back from the GPU.** Rejected for now: the hardware renderer shades vertices on the GPU, so the bounds would need a readback per layer and frame.

## Consequences

A box that is too small no longer hides art; it only makes the frame and hit area smaller than the art. Words still need a box that holds their anchors, and where text layers' boxes overlap the smallest box that holds a string owns it. Moving another Board over means adding it to `UNCUT_BOARDS` and checking each of its pages for parts a node layer owns but that were hidden only by the cut.
