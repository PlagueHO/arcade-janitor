use std::{fs, path::Path};

use crate::{CleanMameError, Result, errors::io_error, models::RomEntry};

pub fn move_roms(
    roms: &[RomEntry],
    target_folder: impl AsRef<Path>,
    dry_run: bool,
) -> Result<Vec<String>> {
    let target_folder = target_folder.as_ref();
    if !dry_run {
        fs::create_dir_all(target_folder).map_err(|source| io_error(target_folder, source))?;
    }

    let mut moved = Vec::new();
    for rom in roms {
        let source = rom
            .rom_path
            .as_ref()
            .ok_or_else(|| CleanMameError::MissingPath(rom.name.clone()))?;
        let file_name = source
            .file_name()
            .ok_or_else(|| CleanMameError::MissingPath(rom.name.clone()))?;
        let target = target_folder.join(file_name);
        if !dry_run {
            fs::rename(source, &target).map_err(|source| io_error(&target, source))?;
        }
        moved.push(rom.name.clone());
    }

    Ok(moved)
}
