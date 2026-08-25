use serde::{Deserialize, Serialize};

use crate::models::{Region, RomEntry};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FilterOptions {
    pub genre_contains: Option<String>,
    pub region: Option<Region>,
    pub include_mature: bool,
    pub include_mechanical: bool,
    pub include_prototype: bool,
    pub only_available: bool,
}

pub fn filter_roms(roms: &[RomEntry], options: &FilterOptions) -> Vec<RomEntry> {
    roms.iter()
        .filter(|rom| matches_options(rom, options))
        .cloned()
        .collect()
}

fn matches_options(rom: &RomEntry, options: &FilterOptions) -> bool {
    if !rom.metadata.flags.runnable {
        return false;
    }

    if options.only_available && rom.rom_path.is_none() {
        return false;
    }

    if !options.include_mature && rom.metadata.flags.mature {
        return false;
    }

    if !options.include_mechanical && rom.metadata.flags.mechanical {
        return false;
    }

    if !options.include_prototype && rom.metadata.flags.prototype {
        return false;
    }

    if let Some(region) = &options.region
        && &rom.metadata.region != region
    {
        return false;
    }

    if let Some(genre) = &options.genre_contains {
        let needle = genre.to_ascii_lowercase();
        let Some(actual) = &rom.metadata.genre else {
            return false;
        };
        let haystack = format!(
            "{} {}",
            actual.category.to_ascii_lowercase(),
            actual
                .subcategory
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase()
        );
        if !haystack.contains(&needle) {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Flags, RomMetadata};

    #[test]
    fn excludes_mature_by_default() {
        let roms = vec![RomEntry {
            name: "adult".to_string(),
            description: None,
            year: None,
            manufacturer: None,
            metadata: RomMetadata {
                flags: Flags {
                    mature: true,
                    ..Flags::default()
                },
                ..RomMetadata::default()
            },
            rom_path: None,
        }];

        assert!(filter_roms(&roms, &FilterOptions::default()).is_empty());
    }

    #[test]
    fn excludes_non_runnable_roms() {
        let roms = vec![RomEntry {
            name: "bios".to_string(),
            description: None,
            year: None,
            manufacturer: None,
            metadata: RomMetadata {
                flags: Flags {
                    runnable: false,
                    ..Flags::default()
                },
                ..RomMetadata::default()
            },
            rom_path: None,
        }];

        assert!(filter_roms(&roms, &FilterOptions::default()).is_empty());
    }
}
