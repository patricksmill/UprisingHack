# KIU-native Options for PC preferences

KIU already exposes Options from Main Menu and Pause, while our PC preferences and binding tools currently live in the Zakuro Frontend Menu or environment variables. We decided to extend KIU's game-owned Options Screens for all supported player-facing configuration, preserving its visual language and original gameplay settings while replacing obsolete 3DS Controls and instructions. A reskinned Frontend Menu would retain a separate interaction model and would not satisfy the intended native experience.

Game-owned settings keep their existing ownership; new PC preferences remain host-owned, shared across save books and unaffected by console save-state loads. Each setting has one authoritative value and player-facing Control, with changes applied through the existing host/core integration rather than duplicate settings implementations. Native Screen drawing and pointer mapping follow the shared Screen Transform/Placement contract. The Frontend Menu remains a deliberate diagnostics/recovery route, not normal pause navigation.

Action-based keyboard/mouse and controller configuration replaces the KIU-facing virtual-3DS binding experience, with the supplied Darkshade C-Stick & Gyro patch as the controller starting point. Patch compatibility, physical gyro transport and native creation of additional Controls must be demonstrated before claiming support; neither a replacement game binary nor an egui player UI is an assumed shortcut. The complete approved scope, migration rules, acceptance scenarios and implementation gates are in [the implementation plan](../KIU_SETTINGS_PLAN.md).

## Status

Abandoned, 16 November 2026: native PC Settings Screens were not worth the effort, and the PC Settings Row prototype was removed. PC Preferences stay in the UprisingHack Window of the Frontend Menu. The parts that did not need native Screens were built and stay: the settings store, Action Bindings and controller gyro.
