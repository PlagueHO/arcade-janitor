use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::{Result, errors::io_error};

pub fn list_rom_files(folder: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
    let folder = folder.as_ref();
    let mut roms = Vec::new();

    for entry in WalkDir::new(folder).min_depth(1).max_depth(1) {
        let entry = entry.map_err(|source| io_error(folder, source.into()))?;
        let path = entry.path();
        if path.is_file() && is_rom_archive(path) {
            roms.push(path.to_path_buf());
        }
    }

    roms.sort();
    Ok(roms)
}

pub fn is_rom_archive(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(extension) if extension.eq_ignore_ascii_case("zip") || extension.eq_ignore_ascii_case("7z")
    )
}
