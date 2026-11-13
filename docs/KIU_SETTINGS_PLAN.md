# KIU-native Options implementation plan

Status (13 Nov): in progress.

| Slice | State |
| --- | --- |
| 1. Audit | Options inventory, patch overlap (3 of 344 hook addresses), battle button probe and the font-string mechanism recorded in `analysis/FINDINGS.md`. No new native Control built yet. Patch not yet installed as a mod. |
| 2. Settings store | Built: `PcPreferences` (aim, presentation, Action Bindings) in the settings file; old files migrate; category Restore Defaults; saves written beside and renamed over, failures shown; the migration notice stays until acknowledged; recovery resets ask first. `ZAKURO_AIM_SENSITIVITY`, `ZAKURO_HUD_SCALE`, `ZAKURO_PORTRAIT_SCALE` removed. Display rollback not built: no Display/Graphics Screen yet. |
| 3. Action Bindings | Built for keyboard, mouse and controller: contexts, primary/secondary, conflicts with Swap/Replace/Cancel, required actions, press-context rule, one start per action across its inputs, release on capture loss, focus loss and controller disconnect, dead zone and right-stick sensitivity (Kid Icarus only). Battle has no held-item input (pickups take effect at once). A, B, Y, ZR and D-pad Up attack as L does, so Attack covers them; X is the game's own Change View, now an action (V, controller top face); R and ZL show no role and are not bound. Vertical inversion stays the game's own. |
| 4. Gyro | SDL3 (static, controllers and sensors only) reads the gyroscope into the console's HID gyroscope, off by default, Always/Hold, calibration, inversion. Aiming with it needs the patch installed. Not checked on physical hardware. |
| 5. Native Screens | Escape/Start reach the game's Pause; F12 or Home opens the Frontend Menu. KIU's Options do not yet show any PC Preference; until they do, the Frontend Menu's Settings has a Kid Icarus recovery section. |
| 6. Instructions | The game's button prompts show the inputs bound to Confirm, Back, Attack, the menu directions and Move, per device. Touch-screen instructions, control diagrams and presentation settings in KIU's Screens not started. |

## Goal and scope

Players configure the PC version inside KIU's Options Screens, reached from Main Menu and Pause. Preserve KIU's artwork, typography, sounds, Outline and transitions while making every supported Control usable with controller and mouse/keyboard. This extends existing in-battle Options access; it does not introduce that ability.

Preserve original gameplay settings, including difficulty. No new gameplay modifiers, difficulty system, generic settings framework, or replacement UI toolchain. The Frontend Menu remains for development, diagnostics and recovery, not routine player configuration. This is not an egui settings window dressed up to resemble KIU.

The implementation is complete only when all workstreams and acceptance scenarios below pass. Ordered slices are integration checkpoints, not permission to ship unfinished settings or call the whole feature complete early. No new dependency is approved by this plan.

## Approved player-facing organization

| Screen | Responsibility | Ownership |
| --- | --- | --- |
| Options | Existing categories plus Display/Graphics and Presentation; preserve meaningful original destinations | Native navigation extended for PC |
| Controls | Keyboard/Mouse Bindings, Controller Bindings, stick behavior | New PC preferences; preserve meaningful original settings |
| Reticle/Camera | Air Aim, Ground Look, sensitivity, inversion, optional gyro and meaningful native camera/reticle settings | Reconcile overlapping effects; no competing Controls |
| Volume | Existing Music/SFX/Voice, plus master volume and mute | Native channel settings; host master gain |
| Subtitles | Existing subtitle settings, including placement | Preserve native ownership |
| Display/Graphics | Window/fullscreen, rendering resolution and supported graphics options | Host preferences |
| Presentation | Curated HUD/reticle scale and supported dialogue/portrait adjustments | Host preferences; not arbitrary layout editing |

Display/Graphics and Presentation are working visible labels. Before implementation freezes labels, fit them to KIU's actual typography and available layout without changing their responsibilities. Numeric ranges must come from observed runtime behavior, not invented sliders.

### Input and navigation

- Normal Esc/Start during gameplay opens native Pause. Back returns one Screen; binding capture consumes cancellation before navigation. Existing non-gameplay native navigation remains meaningful.
- Opening Pause releases mouse capture; resuming restores the previous gameplay capture state without firing an action or synthesizing a stale drag.
- Controller and mouse/keyboard remain usable without an input-mode switch. Deliberate input changes hints; stick drift, mouse jitter and gyro motion do not. Preserve a coherent Outline across handoff.
- Bind gameplay actions, not virtual 3DS buttons. Include the complete applicable action inventory, including power selection/use, held items and vehicle actions; verify names and context against KIU.
- Primary and secondary assignments are separate for keyboard/mouse and controller. Mouse buttons are supported. Removing assignments cannot strand navigation or recovery.
- Same-context binding conflicts offer explicit Swap, Replace or Cancel where meaningful. Context-exclusive reuse is allowed. Define conflicts using actual active contexts, including chords and analog/digital actions, before implementing the Dialog.
- Controller gameplay supports movement on the left stick and aim/look on the right stick, using the supplied Darkshade patch as the starting point. Configure sensitivity, inversion and dead zones; preserve useful original behavior.
- Gyro is optional, disabled by default, supplements stick aim, and offers Always On during gameplay or Hold to Activate. Include activation binding, calibration, sensitivity, inversion and an explanation of supported axes. It never navigates Options or switches hints by itself.
- Hints, control diagrams and relevant gameplay instructions reflect the current device and bindings. Use readable KIU-styled labels/reflow for long key names and unusual buttons; do not retain misleading 3DS artwork.

### Persistence and application

- Original KIU settings retain their existing storage/ownership. New PC preferences apply across save books and survive console save-state loads.
- Each setting has one authoritative value and one player-facing Control. Preserve Music/SFX/Voice and subtitle placement rather than duplicate them under new categories.
- Ordinary changes apply immediately and save automatically. Back does not discard changes. Explain restart-only changes before applying them.
- Risky display changes are provisional until confirmed and revert on timeout. Loss of focus, device/display loss and process interruption must not leave an unconfirmed mode as the next-launch default. Choose a conservative confirmation interval during implementation and show it visibly.
- Restore Defaults requires confirmation and affects only the current category. Keyboard/mouse and controller reset separately. Preserve unrelated preferences and native game progress.
- Preserve compatible existing values on migration. Translate legacy virtual-button bindings only when action mapping is unambiguous; explain ambiguous cases and offer review or defaults for affected Controls only.
- A failed save must not be reported as persisted. Surface an actionable KIU-styled error and retain a usable active configuration; ensure a failed write does not destroy the last valid configuration.
- Device-specific unsupported features show a reason. Unfinished features have no player-facing Control. High Refresh Mode and stereo remain unavailable under current decisions.

## Current evidence and code seams

These are starting anchors, not claims that the new Screens or input behavior already exist. Keep newly discovered addresses and patch disassembly in `analysis/FINDINGS.md`, not the glossary.

| Location | Evidence / intended involvement |
| --- | --- |
| `tools/zakuro/crates/zakuro/src/settings.rs` | Existing host preferences, virtual-button `Keys`/`PadButtons`, TOML load/save; new preferences must reuse the persistence convention |
| `tools/zakuro/crates/zakuro/src/lib.rs` | Applies host settings, creates AimMouse, reads environment preferences, handles Esc, gamepad polling and save-state loads |
| `tools/zakuro/crates/zakuro/src/input.rs` | Current keyboard-to-3DS mapping; migrate affected consumers to action bindings |
| `tools/zakuro/crates/zakuro/src/gamepad.rs` | gilrs buttons/axes, fixed dead zone, device-family hints; physical gyro acquisition not established |
| `tools/zakuro/crates/zakuro/src/menus.rs` | Existing Frontend Menu settings and rebinding; remove the normal-player duplicate path after cutover, retain explicit diagnostic/recovery functions |
| `tools/zakuro/crates/zakuro-core/src/aim_mouse.rs` | Native input hooks, camera behavior, host gestures and Screen interaction |
| `tools/zakuro/crates/zakuro-core/src/aim_mouse/screen_transform.rs` | Existing ScreenSpec registry and Placement contract |
| `tools/zakuro/crates/zakuro-core/src/aim_mouse/menu_top.rs` | Existing Scripted Menu layout, native model/text handling |
| `tools/zakuro/crates/zakuro-core/src/services/hid.rs` | Emulated accelerometer/gyro samples from tilt; not proof of physical sensor input |
| `tools/zakuro/crates/zakuro-core/src/services/prompts.rs` | Existing prompt representations used by keyboard/controller input |
| `tools/kiutrace/src/main.rs` | Native state, interaction, screenshots and save-state scenario harness |
| `analysis/FINDINGS.md`, Darkshade patch section | USA v172 patch map, input/control-settings modifications and widescreen hooks |
| `patch/Kid Icarus Uprising - C-Stick & Gyro Patch/` | Supplied base patch, region variants, optional fixed-aspect variants and author attribution |

Current persistence logs save failures rather than returning a player-visible result and writes directly to its destination. This must be addressed for the approved autosave/error contract. Current environment-controlled presentation preferences are startup inputs, not proof of live-adjustable settings.

## Ordered implementation slices

### 1. Audit native Options, patch behavior and settings ownership

Read the supplied Emulator installation instructions, base-patch behavior and control artwork before designing replacements. Record provenance, supported game revision, expected instruction words and any reuse/distribution permission questions. Work with the USA build; do not expand regional support merely because patch variants exist.

Build an exhaustive inventory of native Options destinations and host player-facing settings: current label, behavior, owner, storage, default, valid range, availability during battle, live/restart application, replacement/removal, and acceptance scenario. Include native child Screens and both entry paths. Classify runtime-only knobs as diagnostics rather than expose everything in Settings.

Identify actual Screen construction, dynamic text, input and font/artwork paths for adding categories, Rows, sliders and Dialogs. Prototype one real additional host-backed Control through the game-owned drawing/input path and observe it from Main Menu and Pause. Do not assume moving existing models creates new interactive Controls.

Compare base-patch input/control-settings changes against existing mouse, widescreen, vehicle and 60 FPS hooks. Establish a compatibility strategy from instruction/data evidence. Do not blindly install replacement code.bin or combine the fixed-aspect patch with our arbitrary-aspect projection hooks. Preserve patch attribution; do not redistribute game binaries.

The supplied Emulator instructions identify v172 as v163 plus Circle Pad aiming through the C-Stick assistance path. Touch input retains priority; the documented preset uses a movement assignment in Step 1 and Touch Screen & Circle Pad in Step 2. Its executable/header and matching RomFS assets form a set, and it requires a normal launch rather than an older save state. Matching RomFS directories are present locally. Verify these dependencies and separate intended assistance/lock/hopping behavior from unrelated patch changes before combining them with our mouse path; do not interpret “use the patch” as approval to change gameplay rules. Several detailed revision notes referenced by INSTALLATION.md were not found under the supplied patch directory; recover their behavior from available assets/code or obtain the missing notes before relying on their claims.

Exit: complete inventory, source-backed native Screen feasibility proof, patch integration map, representative screenshots, and concrete rendering/input approach recorded in findings. If native construction needs another mechanism, explicitly update this plan/decision before proceeding; no silent substitution of an egui player UI.

### 2. Establish authoritative preference changes and migration

Extend existing settings storage rather than introduce another config toolchain. Give PC preferences explicit defaults and validity rules, and expose changes/results through the existing host/core integration seam. Keep platform display/device operations host-owned and native game settings game-owned; do not make the core depend on windowing or controller libraries. Use a small concrete interface only where required; no generic plugin/adapter framework.

Implement migration, durable saves/error reporting, live application, category reset and provisional display rollback. Convert player-facing environment preferences to stored settings and update every affected reader. Retain documented technical compatibility flags, but do not let a hidden environment value silently override the value displayed in Options. Any remaining diagnostic override must be explicit and observable.

Reapply host-owned preferences after console state loads without overwriting native settings. Preserve unrelated existing host preferences and other-title configuration; KIU-specific migration must not globally rewrite shared virtual-button mappings for other games.

Exit: real runtime adjustments persist across restart and state load; category reset and failed-save behavior are demonstrated; old configurations migrate without unrelated data loss.

### 3. Implement action bindings and supplied controller behavior

Enumerate real gameplay/navigation contexts and their actions first. Migrate all affected keyboard, mouse, controller, prompt and native-input consumers; remove obsolete KIU binding aliases after migration. Resolve simultaneous primary/secondary sources into one action state: one source releasing must not release an action another source still holds. Preserve analog range, native repeat, edge-triggered actions and context-specific behavior.

Integrate patch-derived dual-stick behavior with existing Air Aim/Ground Look and vehicles. Verify stick response/dead zones and inversion across air, land, transitions and camera-restricted scenes. Avoid treating labels alone as action support; implement missing power/item paths required by the complete action inventory.

Implement conflict detection, capture cancellation and reconnect behavior. Clear pending input on focus loss, Screen changes, loads and disconnects; preserve settings. Deliberate-input ownership excludes gyro and drift. Recovery navigation cannot depend on a binding the player can erase.

Exit: complete action inventory works on actual gameplay paths with each input method, including simultaneous sources, held inputs, vehicles and handoff. No stale touch/action survives a transition.

### 4. Connect physical controller gyro

Investigate the existing controller backend/platform capabilities before selecting sensor transport. Inspect the supplied patch's sensor assumptions; validate axes, units, sampling and calibration with actual device readings. If this needs a dependency or new architectural module, present the evidence and obtain approval before adding it. Approval of gyro as a feature is not dependency approval.

Implement additive gyro plus stick behavior, activation mode/binding, calibration and clear unsupported-device status. Define supported controller/platform combinations from observed hardware results, not controller names alone. Reconnect and focus changes must not introduce a large first-sample turn.

Exit: physical Windows controller motion produces the expected in-game response in air and land; disabled/held states are checked; Options remains still; unsupported devices and disconnect/reconnect behave honestly. Sensor injection can supplement verification, not replace physical hardware proof.

### 5. Extend KIU Options Screens and normal navigation

Add or extend the approved Screens, Rows, sliders and Dialogs using the proven native mechanism. Register game-owned Screens in the Screen Transform process; one resolved Placement list is shared by drawing and pointer mapping. Native keyboard/controller navigation, Outline, sound and transitions remain coherent. Preserve the existing native Options access from Pause and Main Menu.

Route normal pause inputs to native Pause, with Back/capture cancellation precedence. Keep the Frontend Menu only on a deliberate diagnostic/recovery route, documented and usable when game Options cannot be reached. Delete obsolete normal-player settings/rebinding routes after all replacements work; do not delete unrelated emulator/development facilities.

Use existing player settings application logic as the authority, not independent copies for Pause and Scripted Menus. Availability/restart text reflects actual capabilities. Sliders support controller adjustment, mouse track clicks and dragging. Wheel behavior is explicit for scrollable Lists and does not accidentally change a neighboring value.

Exit: every inventory setting is reachable and effective from both entry paths as applicable, without opening the Frontend Menu. Every game-owned Screen satisfies the Screen Transform behavior contract or has an explicit, justified exception.

### 6. Update instructions and bounded presentation settings

Replace stale touchscreen instructions and device diagrams where they describe affected actions. Resolve prompts against active bindings/controller family, including alternatives, long key names and missing bindings. Keep hints readable at narrow and ultrawide widths; preserve appropriate native text/localization behavior. Audit localized assets before claiming language support; do not silently narrow existing supported text behavior.

Expose only demonstrated HUD/reticle/dialogue/portrait adjustments with useful ranges and previews. Preserve native subtitle placement in Subtitles. Keep arbitrary Placement/visibility/navigation editing behind development tooling. Hiding a setting or decoration must not hide required gameplay information accidentally.

Exit: screenshots and interactions show correct instructions and curated presentation behavior across the required widths and both devices. No stale diagram or hidden override contradicts the active configuration.

## Canonical change requests

Split multi-Control work into independently verifiable requests following `docs/agents/change-requests.md`. The following define the major transformations; the inventory supplies the remaining original Controls.

- **Add Display/Graphics and Presentation Controls on Options:** existing category destinations → existing destinations plus the two PC categories. **Acceptance:** both Main Menu and Pause entry paths open each destination and return to their origin without changing game progress.
- **Add Keyboard/Mouse Bindings and Controller Bindings Controls on Controls:** virtual-3DS-oriented configuration → separate action-based binding destinations. **Acceptance:** rebind Attack with each device, activate it in gameplay, restart, and verify the assignments remain.
- **Add master-volume and mute Controls on Volume:** native channel controls only → native channel controls plus host master gain. **Acceptance:** change master gain/mute audibly while Music/SFX/Voice values remain unchanged.
- **Add gyro Controls on Reticle/Camera:** no established physical-controller gyro route → optional calibrated stick-plus-gyro aiming. **Acceptance:** a supported physical controller aims in both battle types only under the configured activation policy; unsupported hardware shows its limitation.
- **Rebind pause activation on Pause:** Esc opens the Frontend Menu → normal Esc/Start opens native Pause. **Acceptance:** pause and resume with both devices without firing Attack or retaining a held Synthetic Touch; Back unwinds Options before resuming.

New Screen content receives separate add requests naming its Controls before implementation; adding a navigation destination alone does not satisfy the feature.

## Acceptance matrix

| Scenario | Required observable result |
| --- | --- |
| Main Menu versus Pause | Same supported categories and values; original subtitles/controls remain editable in battle; return restores the originating Screen |
| Mouse/controller handoff | Hover, Cursor and hints follow deliberate input; drift/gyro do not switch ownership; no duplicate activation |
| Binding capture | Keys, mouse buttons, controller primary/secondary assignments work; cancel leaves previous assignment intact; conflicts explicit |
| Held inputs and contexts | Secondary-source release does not cancel a still-held primary; edge actions fire once; valid context reuse does not trigger inactive actions |
| Gameplay coverage | Air, land, vehicle ability/exit, powers, items, camera restrictions and transitions use the displayed actions correctly |
| Gyro hardware | Calibration, axes, sensitivity, inversion, held/always modes, unsupported hardware and reconnect verified on supported Windows hardware |
| Save books/state loads | PC preferences remain identical across books and console state loads; original settings preserve native ownership |
| Migration | Valid prior preferences retained; ambiguous mappings explained; unrelated preferences untouched; interrupted/failed persistence does not destroy the last valid file |
| Category defaults | Confirmation resets only the category/device requested; Cancel changes nothing |
| Display rollback | Unconfirmed mode reverts; timeout/focus loss/interruption/relaunch cannot strand the user; confirmed mode persists |
| Failure reporting | Failed persistence/device operation is visible and does not falsely report success or display a value different from the active state |
| Widths | 5:3, 16:9, 21:9 and a narrow window: labels readable, Outline and pointer agree, Lists scroll, sliders remain operable |
| Instructions | Rebinding and device handoff update affected Hint strips/diagrams; long labels remain readable; no misleading touch-only directions |
| Recovery | Unusable game settings can be recovered without entering Options; normal players never need the Frontend Menu |

## Verification and delivery

Use the existing Cargo suites and kiutrace runner. Run affected crate tests and the integrated workspace checks using repository conventions; do not add another test runner. Permanent tests cover consumer-visible transitions, conflict/migration boundaries and error behavior, not descriptor/source-text wiring. Use throwaway probes for native construction and remove them after capturing findings.

For each implementation slice, run the changed runtime path; suites alone are not proof. Existing save-state scenarios and new behavior-focused kiutrace commands cover native interaction. Launch the real application for host windows, binding capture, live sound, controller prompts and gyro. Capture the actual Options Screens at required widths. Refresh the harness build first: an earlier comparison smoke used an old binary that did not recognize `stereo`.

After code changes, rebuild the Windows release executable using the repository's working cross-build procedure, replace `dist/zakuro-kiu-mouse.exe`, and compare its checksum with the new build. Discover the actual current build command rather than invent one in this document. Report build or physical Windows verification blockers explicitly; headless sensor injection is not hardware verification. Update user documentation, controls instructions, patch attribution and this plan's status as behavior lands.

## Remaining engineering prerequisites

- Native addition of Controls/Screens and dynamic text must be demonstrated, not inferred from existing relocation support.
- Patch compatibility and allowed reuse of authored code/artwork require inspection; do not ship replacement game code or assume redistribution permission from attribution alone.
- Physical controller gyro transport, supported devices and Windows hardware access remain unverified. A needed dependency requires approval.
- Exact action inventory, value ranges, reset defaults, restart-only settings and migration ambiguities are audit outputs.
- These prerequisites do not reduce approved scope. If any cannot be satisfied, report evidence and obtain an explicit scope/design decision rather than ship no-op Controls or an undisclosed Frontend Menu substitute.
