# 9. Dynamic Input Textures Are Drawn in Memory over the Game's Texture

The font's button symbols follow the Action Bindings, but some of the game's button prompts are pictures in its textures: the battle HUD's round A, B, X and Y, the Control Configuration diagrams, the X and Y pills. A keyboard player sees a 3DS's buttons there. Dolphin solves the same problem with Dynamic Input Textures: a JSON config gives regions of a texture per emulated button and a picture per host key, and Dolphin writes a custom texture with the bound keys' pictures in those regions.

We decided to read Dolphin's config format (specification 1) and draw the results in memory, on the GPU's texture lookup, in front of the texture pack. The base is the game's own texture as the GPU decodes it, scaled up, unless the config gives an `image`; a key without a picture is drawn the way the font's symbols are. Zakuro's own config for the game therefore holds only texture hashes and rectangles (`kiu/dynamic_input_textures/kiu.json`), and the repository stays free of the game's art. The set is made again when the button symbols change (`System::set_prompts`), salted so the GPU, which keeps uploaded pictures by hash, takes them as new, and the texture cache re-picks each texture's picture without decoding it again.

## Considered options

- **Write PNGs into the texture pack folder, as Dolphin writes its generated folder.** Rejected: it needs the game's texture as a file to start from, which the repository cannot ship, it rewrites files on every device switch, and the pack would have to be read again.
- **Ship redrawn HD bases with the config.** Rejected for the built-in config (game art); still possible for a player's own config through `image`.
- **Patch the textures in guest memory.** Rejected: the CPU-side texture bytes are the game's state, a save state would carry the patch, and the software renderer is the only place it would add anything.

## Consequences

Dynamic Input Textures draw only on the host's GPU, like texture packs, and only with Texture packs on. A config's regions are measured in a RomFS dump (`kiu/scripts/kiu_textures.py`, which now notes each texture's pack name) and checked with `ditpreview` and `kiutrace`'s `prompts` command. START and SELECT are not inputs of the button symbols, so textures with `(START)` baked into their lettering stay as they are.
