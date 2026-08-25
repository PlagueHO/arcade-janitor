use std::{collections::HashMap, path::Path};

use crate::{
    Result,
    models::{Genre, RomEntry},
    parsers::{
        catver::parse_catver_file,
        mame_xml::{parse_mame_xml_file, parse_mame_xml_str},
    },
    utils::{filesystem::list_rom_files, paths::rom_name_from_path},
};

pub fn load_metadata(
    mame_xml: impl AsRef<Path>,
    catver: Option<impl AsRef<Path>>,
) -> Result<Vec<RomEntry>> {
    let mut roms = parse_mame_xml_file(mame_xml)?;
    if let Some(catver) = catver {
        apply_genres(&mut roms, &parse_catver_file(catver)?);
    }
    Ok(roms)
}

pub fn load_metadata_from_str(
    mame_xml: &str,
    catver: Option<impl AsRef<Path>>,
) -> Result<Vec<RomEntry>> {
    let mut roms = parse_mame_xml_str(mame_xml)?;
    if let Some(catver) = catver {
        apply_genres(&mut roms, &parse_catver_file(catver)?);
    }
    Ok(roms)
}

pub fn scan_rom_folder(
    rom_folder: impl AsRef<Path>,
    mame_xml: impl AsRef<Path>,
    catver: Option<impl AsRef<Path>>,
) -> Result<Vec<RomEntry>> {
    let roms = load_metadata(mame_xml, catver)?;
    scan_rom_folder_with_entries(rom_folder, roms)
}

pub fn scan_rom_folder_with_entries(
    rom_folder: impl AsRef<Path>,
    mut roms: Vec<RomEntry>,
) -> Result<Vec<RomEntry>> {
    let mut by_name: HashMap<String, usize> = roms
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.name.to_ascii_lowercase(), index))
        .collect();

    for path in list_rom_files(rom_folder)? {
        if let Some(name) = rom_name_from_path(&path) {
            if let Some(index) = by_name.get(&name) {
                roms[*index].rom_path = Some(path);
            } else {
                let mut entry = RomEntry::new(&name);
                entry.rom_path = Some(path);
                by_name.insert(name, roms.len());
                roms.push(entry);
            }
        }
    }

    Ok(roms)
}

pub fn find_by_name<'a>(roms: &'a [RomEntry], name: &str) -> Option<&'a RomEntry> {
    roms.iter().find(|rom| rom.name.eq_ignore_ascii_case(name))
}

fn apply_genres(roms: &mut [RomEntry], genres: &HashMap<String, Genre>) {
    for rom in roms {
        if let Some(genre) = genres.get(&rom.name.to_ascii_lowercase()) {
            rom.metadata.genre = Some(genre.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Genre;

    #[test]
    fn finds_rom_case_insensitively() {
        let roms = vec![RomEntry::new("pacman")];

        assert!(find_by_name(&roms, "PACMAN").is_some());
    }

    #[test]
    fn applies_genres_by_name() {
        let mut roms = vec![RomEntry::new("pacman")];
        let genres = HashMap::from([(
            "pacman".to_string(),
            Genre {
                category: "Maze".to_string(),
                subcategory: None,
            },
        )]);

        apply_genres(&mut roms, &genres);

        assert_eq!(roms[0].metadata.genre.as_ref().unwrap().category, "Maze");
    }
}
