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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_deletion_without_removing_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pacman.zip");
        fs::write(&path, b"rom").unwrap();
        let mut rom = RomEntry::new("pacman");
        rom.rom_path = Some(path.clone());

        assert_eq!(delete_roms(&[rom], true).unwrap(), vec!["pacman"]);
        assert!(path.is_file());
    }

    #[test]
    fn deletes_files_and_reports_missing_paths() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pacman.zip");
        fs::write(&path, b"rom").unwrap();
        let mut rom = RomEntry::new("pacman");
        rom.rom_path = Some(path.clone());

        assert_eq!(delete_roms(&[rom], false).unwrap(), vec!["pacman"]);
        assert!(!path.exists());

        let error = delete_roms(&[RomEntry::new("missing")], false).unwrap_err();
        assert!(matches!(error, ArcadeJanitorError::MissingPath(name) if name == "missing"));
    }

    #[test]
    fn reports_filesystem_errors() {
        let mut rom = RomEntry::new("missing");
        rom.rom_path = Some(std::env::temp_dir().join("arcadejanitor-no-such-rom.zip"));

        let error = delete_roms(&[rom], false).unwrap_err();
        assert!(matches!(
            error,
            ArcadeJanitorError::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound
        ));
    }
}
