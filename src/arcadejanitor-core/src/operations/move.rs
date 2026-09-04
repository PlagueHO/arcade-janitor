use std::{
    fs::{self, File, OpenOptions},
    io,
    path::Path,
};

use crate::{ArcadeJanitorError, Result, errors::io_error, models::RomEntry};

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
            .ok_or_else(|| ArcadeJanitorError::MissingPath(rom.name.clone()))?;
        let file_name = source
            .file_name()
            .ok_or_else(|| ArcadeJanitorError::MissingPath(rom.name.clone()))?;
        let target = target_folder.join(file_name);
        if dry_run {
            ensure_target_available(&target)?;
        } else {
            move_file(source, &target)?;
        }
        moved.push(rom.name.clone());
    }

    Ok(moved)
}

fn move_file(source: &Path, target: &Path) -> Result<()> {
    let mut reservation = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|error| io_error(target, error))?;

    match fs::rename(source, target) {
        Ok(()) => {
            drop(reservation);
            Ok(())
        }
        Err(_rename_error) => {
            let result = (|| {
                let mut source_file =
                    File::open(source).map_err(|error| io_error(source, error))?;
                io::copy(&mut source_file, &mut reservation)
                    .map_err(|error| io_error(target, error))?;
                fs::remove_file(source).map_err(|error| io_error(source, error))
            })();
            if result.is_err() {
                drop(reservation);
                let _ = fs::remove_file(target);
            }
            result
        }
    }
}

fn ensure_target_available(target: &Path) -> Result<()> {
    if target
        .try_exists()
        .map_err(|error| io_error(target, error))?
    {
        return Err(io_error(
            target,
            io::Error::new(io::ErrorKind::AlreadyExists, "target file already exists"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_replace_existing_target() {
        let folder =
            std::env::temp_dir().join(format!("arcadejanitor-move-{}", std::process::id()));
        let source = folder.join("source.zip");
        let target = folder.join("target.zip");
        fs::create_dir_all(&folder).unwrap();
        fs::write(&source, "source").unwrap();
        fs::write(&target, "target").unwrap();

        let error = move_file(&source, &target).unwrap_err();

        assert!(matches!(
            error,
            ArcadeJanitorError::Io {
                source,
                ..
            } if source.kind() == io::ErrorKind::AlreadyExists
        ));
        assert_eq!(fs::read_to_string(&source).unwrap(), "source");
        assert_eq!(fs::read_to_string(&target).unwrap(), "target");
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn previews_move_without_creating_target_folder() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.zip");
        let target_folder = directory.path().join("moved");
        fs::write(&source, b"source").unwrap();
        let mut rom = RomEntry::new("source");
        rom.rom_path = Some(source.clone());

        assert_eq!(
            move_roms(&[rom], &target_folder, true).unwrap(),
            vec!["source"]
        );
        assert!(source.is_file());
        assert!(!target_folder.exists());
    }

    #[test]
    fn moves_files_and_reports_missing_paths() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.zip");
        let target_folder = directory.path().join("moved");
        fs::write(&source, b"source").unwrap();
        let mut rom = RomEntry::new("source");
        rom.rom_path = Some(source.clone());

        assert_eq!(
            move_roms(std::slice::from_ref(&rom), &target_folder, false).unwrap(),
            vec!["source"]
        );
        assert!(!source.exists());
        assert_eq!(
            fs::read(target_folder.join("source.zip")).unwrap(),
            b"source"
        );

        let error = move_roms(&[RomEntry::new("missing")], &target_folder, true).unwrap_err();
        assert!(matches!(error, ArcadeJanitorError::MissingPath(name) if name == "missing"));
    }
}
