use std::{
    fs::{self, File, OpenOptions},
    io,
    path::Path,
};

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
        let folder = std::env::temp_dir().join(format!("cleanmame-move-{}", std::process::id()));
        let source = folder.join("source.zip");
        let target = folder.join("target.zip");
        fs::create_dir_all(&folder).unwrap();
        fs::write(&source, "source").unwrap();
        fs::write(&target, "target").unwrap();

        let error = move_file(&source, &target).unwrap_err();

        assert!(matches!(
            error,
            CleanMameError::Io {
                source,
                ..
            } if source.kind() == io::ErrorKind::AlreadyExists
        ));
        assert_eq!(fs::read_to_string(&source).unwrap(), "source");
        assert_eq!(fs::read_to_string(&target).unwrap(), "target");
        fs::remove_dir_all(folder).unwrap();
    }
}
