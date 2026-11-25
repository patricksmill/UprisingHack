# Screen status

One row per Screen (names from `GLOSSARY.md`), one column per milestone.
A row with no ✅ hasn't been touched. Agents: pick a cell, do the work, and
update the row in the same commit.

## Columns

| Column | Ticked when |
| --- | --- |
| **Layerized** | The Screen is drawn on the top screen through the Screen Transform, and its parts are separate editor layers. |
| **Controller** | Keys and gamepad move the Outline across every Control and A activates the outlined one, on the top screen. |
| **Mouse** | Hover outlines the Control under the pointer and a click activates it (or makes the native touch, for touch-only parts). Wheel scrolls Lists. |
| **Rearranged** | The default layout has been redone for a single wide screen rather than copying the 3DS's two screens, and reviewed at 5:3, 16:9 and 21:9. |

Marks: ✅ done · ◐ partial (see Notes) · ☐ not done · ? not verified.

First pass: 25 November 2026, from PLAN.md, FINDINGS, git history and the
layer counts in `kiu/menu-layout.toml`; not from a play-through. Treat `?`
as "look first". Nothing has been checked with a real mouse on Windows yet.

## In-Game Menus

| Screen | Layerized | Controller | Mouse | Rearranged | Notes |
| --- | --- | --- | --- | --- | --- |
| Pause | ✅ | ✅ | ✅ | ? | Split into layers (`[pause]` rules). |
| Retreat Dialog | ✅ | ✅ | ✅ | ? | Its own `DialogRules`; moved No click checked on `trace/s5.state`. |
| Midstage Loading | ✅ | — | — | ✅ | No Controls. Four layers, apart by default. |
| Control Configuration | ☐ | ? | ◐ | ☐ | No `ScreenId`; Step 1 has input hooks only. |

## Scripted Menus

| Screen | Layerized | Controller | Mouse | Rearranged | Notes |
| --- | --- | --- | --- | --- | --- |
| File Select | ✅ | ✅ | ✅ | ✅ | Board and backdrop are separate layers. |
| Chapter Select | ✅ | ✅ | ✅ | ✅ | Rows in one row; wheel glides it; keys pull it back to the Outline. |
| Main Menu | ✅ | ✅ | ✅ | ? | StreetPass/Together/AR out of scope. |
| Options | ✅ | ✅ | ✅ | ? | |
| Other | ✅ | ✅ | ✅ | ? | Labels are baked textures. |
| Controls | ✅ | ✅ | ✅ | ? | |
| Reticle/Camera | ☐ | ✅ | ✅ | ☐ | No layers. |
| Subtitles | ☐ | ✅ | ✅ | ☐ | No layers; label is a model texture. |
| Volume | ✅ | ✅ | ✅ | ? | |
| Profile | ◐ | ✅ | ✅ | ☐ | One layer only. |
| Profile Yes/No | ☐ | ✅ | ✅ | ☐ | |
| SpotPass Dialog | ◐ | ✅ | ✅ | ☐ | One layer. |
| Solo Hub | ✅ | ✅ | ✅ | ✅ | Drag physics in the displayed canvas. |
| Arms Altar | ✅ | ✅ | ✅ | ? | |
| Armory — Gear | ✅ | ✅ | ✅ | ? | |
| Armory — Redeem Hearts | ? | ✅ | ✅ | ? | No toml rows of its own; may share Gear's. |
| Armory — Convert to Hearts | ? | ✅ | ✅ | ? | Same as Redeem Hearts. |
| Armory — Fuse Weapons | ✅ | ✅ | ✅ | ✅ | Layers in code; both screens split. |
| Armory — Powers | ✅ | ✅ | ✅ | ? | Layers in code (`POWERS_TOP`); entry froze once (fixed 16 Nov), recheck. |
| Armory — Weapons | ✅ | ✅ | ✅ | ? | Category filter is a Dialog. |
| Armory — Categories | ✅ | ✅ | ✅ | ? | |
| Practice Range | ✅ | ✅ | ✅ | ? | |
| Idol Toss (Solo) | ◐ | ? | ◐ | ✅ | Parts are touch-only (click = touch). Mist and black ground off; `placed_scripted_text_omits_the_same_colour_offset_copy` fails. |
| Idol Toss (Vault) | ◐ | ? | ◐ | ✅ | Same as Solo. |
| Treasure Hunt | ✅ | ? | ◐ | ? | Tiles are touch-only. |
| Vault | ✅ | ✅ | ✅ | ? | |
| Idols | ✅ | ◐ | ✅ | ✅ | Idol View orbits on a stick; the viewer has no Outline of its own. |
| Power Portrait | ◐ | ☐ | ◐ | ☐ | Top split recorded; no Outline, a click is a touch. |
| Chapter Results | ✅ | ✅ | ✅ | ✅ | Bottom node layers drawn whole. |
| Music Gallery | ◐ | ✅ | ✅ | ✅ | Screen 29. Back is part of the Background layer, so it can't move alone. Layout authored at 16:9; at 21:9 a thin black strip remains on the right. |
| Fiend's Cauldron | ✅ | ✅ | ✅ | ✅ | Own F2 editor; dial drag checked. |

## Not registered yet

| Screen | Layerized | Controller | Mouse | Rearranged | Notes |
| --- | --- | --- | --- | --- | --- |
| Offering | ☐ | ? | ☐ | ☐ | Only the dial depth fix. |
| Records | ☐ | ? | ☐ | ☐ | |
| AR (Vault, Main Menu) | ☐ | ☐ | ☐ | ☐ | Needs AR cards; probably `wontfix`. |

## Across all Screens

- Overrides aren't rules that hold at every width yet (PLAN, "Screen Transform pipeline").
- Loading a save state made at another width compounds the widening.
