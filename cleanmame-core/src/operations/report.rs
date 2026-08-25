use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::models::RomEntry;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub total: usize,
    pub available: usize,
    pub by_genre: BTreeMap<String, usize>,
}

pub fn generate_report(roms: &[RomEntry]) -> Report {
    let mut by_genre = BTreeMap::new();
    for rom in roms {
        let genre = rom
            .metadata
            .genre
            .as_ref()
            .map(|genre| genre.category.clone())
            .unwrap_or_else(|| "Unknown".to_string());
        *by_genre.entry(genre).or_insert(0) += 1;
    }

    Report {
        total: roms.len(),
        available: roms.iter().filter(|rom| rom.rom_path.is_some()).count(),
        by_genre,
    }
}
