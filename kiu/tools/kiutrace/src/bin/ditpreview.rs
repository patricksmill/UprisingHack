//! Previews Dynamic Input Textures without running the game.
//!
//! usage: ditpreview <config dir> <texture dump dir> <out dir> [keyboard|controller]
//!
//! Reads the configs in the config dir as the game would (with Kid Icarus'
//! built-in one first when the dir is `-`), finds each texture they name
//! in a `kiu/scripts/kiu_textures.py` dump by the `pack` line of its notes,
//! and writes `<name>.png`: the texture with sample bindings drawn over its
//! buttons (keyboard: A E, B Esc, X V, Y Q, L Left Click, R Right Click,
//! D-Pad 1234, circle pad WASD; controller: a PlayStation pad's).

use std::path::Path;

use zakuro_core::dynamic_textures::Config;

#[path = "../sample_prompts.rs"]
mod sample_prompts;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [config, dumps, out, rest @ ..] = &args[..] else {
        eprintln!("usage: ditpreview <config dir or -> <texture dump dir> <out dir> [keyboard|controller]");
        std::process::exit(2);
    };
    let controller = rest.first().is_some_and(|host| host == "controller");
    let config = match config.as_str() {
        "-" => Config::for_title(0x0004_0000_0003_0100, None),
        dir => Config::load(Path::new(dir)),
    };
    let prompts = sample_prompts::sample(controller);
    let Some(built) = config.build(&prompts) else {
        eprintln!("the configs draw nothing for these bindings");
        return;
    };
    std::fs::create_dir_all(out).expect("make the out dir");
    let mut written = 0;
    for entry in std::fs::read_dir(dumps)
        .expect("read the dump dir")
        .flatten()
    {
        let notes = entry.path();
        if notes.extension().is_none_or(|extension| extension != "txt") {
            continue;
        }
        let text = std::fs::read_to_string(&notes).unwrap_or_default();
        let Some(pack) = text.lines().find_map(|line| line.strip_prefix("pack ")) else {
            continue;
        };
        let Some(texture) =
            zakuro_gpu::pack::hash_in_name(pack).and_then(|hash| built.texture(hash))
        else {
            continue;
        };
        let image = zakuro_gpu::pack::read_image(&notes.with_extension("png"))
            .expect("read the dumped texture");
        let texels: Vec<[u8; 4]> = image
            .rgba
            .chunks_exact(4)
            .map(|texel| [texel[0], texel[1], texel[2], texel[3]])
            .collect();
        let made = zakuro_gpu::dynamic::compose(texture, &texels, image.width, image.height);
        let name = notes.file_stem().expect("a name").to_string_lossy();
        let path = Path::new(out).join(format!("{name}.png"));
        let file = std::io::BufWriter::new(std::fs::File::create(&path).expect("make the picture"));
        let mut encoder = png::Encoder::new(file, made.width, made.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .and_then(|mut writer| writer.write_image_data(&made.rgba))
            .expect("write the picture");
        println!("{name} ({pack}) -> {}", path.display());
        written += 1;
    }
    println!("{written} textures");
}
