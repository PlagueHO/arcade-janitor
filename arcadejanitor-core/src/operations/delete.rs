use std::fs;

use crate::{ArcadeJanitorError, Result, errors::io_error, models::RomEntry};

pub fn delete_roms(roms: &[RomEntry], dry_run: bool) -> Result<Vec<String>> {
    let mut deleted = Vec::new();
    for rom in roms {
        let path = rom
            .rom_path
            .as_ref()
            .ok_or_else(|| ArcadeJanitorError::MissingPath(rom.name.clone()))?;
        if !dry_run {
            fs::remove_file(path).map_err(|source| io_error(path, source))?;
        }
        deleted.push(rom.name.clone());
    }

    Ok(deleted)
}
