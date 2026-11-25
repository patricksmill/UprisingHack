//! The port's own folders, apart from Zakuro's, so stock Zakuro and other
//! Zakuro-based builds don't rewrite its settings or recompile its games.

use std::path::{Path, PathBuf};

/// the folder's name in each of the system's places (data, settings, cache, tools)
pub const NAME: &str = "UprisingHack";

/// the name the port used before, shared with stock Zakuro
const OLD: &str = "zakuro";

/// `base`/UprisingHack, first copied from `base`/zakuro when only that one exists,
/// so a player's settings, mods and recompiled games come along; Zakuro's stays
pub fn in_place(base: &Path) -> PathBuf {
    let dir = base.join(NAME);
    let old = base.join(OLD);
    if !dir.exists() && old.is_dir() {
        // copied under another name first, a copy cut short is never taken for a finished one
        let partial = base.join(format!("{NAME}.partial"));
        let _ = std::fs::remove_dir_all(&partial);
        match copy_dir(&old, &partial).and_then(|()| std::fs::rename(&partial, &dir)) {
            Ok(()) => log::info!("copied {} to {}", old.display(), dir.display()),
            Err(error) => {
                log::warn!("could not copy {} to {}, {error}, starting afresh", old.display(), dir.display());
                let _ = std::fs::remove_dir_all(&partial);
            }
        }
    }
    dir
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("uprisinghack-app-dir-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn copies_zakuro_s_folder_once() {
        let base = scratch("copies");
        std::fs::create_dir_all(base.join("zakuro/mods/x")).unwrap();
        std::fs::write(base.join("zakuro/settings.toml"), "a").unwrap();
        std::fs::write(base.join("zakuro/mods/x/code.ips"), "b").unwrap();
        let dir = in_place(&base);
        assert_eq!(dir, base.join("UprisingHack"));
        assert_eq!(std::fs::read_to_string(dir.join("settings.toml")).unwrap(), "a");
        assert_eq!(std::fs::read_to_string(dir.join("mods/x/code.ips")).unwrap(), "b");
        assert!(base.join("zakuro/settings.toml").exists());
        // later changes to Zakuro's don't come across again
        std::fs::write(base.join("zakuro/settings.toml"), "c").unwrap();
        in_place(&base);
        assert_eq!(std::fs::read_to_string(dir.join("settings.toml")).unwrap(), "a");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn nothing_to_copy() {
        let base = scratch("fresh");
        assert_eq!(in_place(&base), base.join("UprisingHack"));
        assert!(!base.join("UprisingHack").exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
