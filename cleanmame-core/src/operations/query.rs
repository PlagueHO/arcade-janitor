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
    roms: Vec<RomEntry>,
) -> Result<Vec<RomEntry>> {
    scan_rom_folder_with_entries_and_progress(rom_folder, roms, |_, _| {})
}

pub fn scan_rom_folder_with_entries_and_progress(
    rom_folder: impl AsRef<Path>,
    mut roms: Vec<RomEntry>,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<Vec<RomEntry>> {
    let mut by_name: HashMap<String, usize> = roms
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.name.to_ascii_lowercase(), index))
        .collect();

    let paths = list_rom_files(rom_folder)?;
    let total = paths.len() as u64;
    on_progress(0, total);
    for (index, path) in paths.into_iter().enumerate() {
        if let Some(name) = rom_name_from_path(&path) {
            if let Some(index) = by_name.get(&name) {
                roms[*index].rom_path = Some(path);
            } else {
                let mut entry = RomEntry::new(&name);
                entry.metadata.flags.runnable = false;
                entry.catalogued = false;
                entry.rom_path = Some(path);
                by_name.insert(name, roms.len());
                roms.push(entry);
            }
        }
        on_progress(index as u64 + 1, total);
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
            rom.metadata.flags.mature |= is_mature_genre(genre);
        }
    }
}

fn is_mature_genre(genre: &Genre) -> bool {
    genre.category.eq_ignore_ascii_case("mature")
        || genre
            .subcategory
            .as_deref()
            .is_some_and(|subcategory| subcategory.eq_ignore_ascii_case("mature"))
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

    #[test]
    fn marks_mature_catver_genres() {
        let mut roms = vec![RomEntry::new("adultgame")];
        let genres = HashMap::from([(
            "adultgame".to_string(),
            Genre {
                category: "Mature".to_string(),
                subcategory: None,
            },
        )]);

        apply_genres(&mut roms, &genres);

        assert!(roms[0].metadata.flags.mature);
    }
}
