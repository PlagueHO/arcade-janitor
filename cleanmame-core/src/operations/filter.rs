use serde::{Deserialize, Serialize};

use crate::models::{Region, RomEntry};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FilterOptions {
    pub genre_contains: Option<String>,
    pub region: Option<Region>,
    pub names: Vec<String>,
    pub genres: Vec<String>,
    pub categories: Vec<String>,
    pub subcategories: Vec<String>,
    pub regions: Vec<Region>,
    pub manufacturers: Vec<String>,
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    pub include_mature: bool,
    pub include_mechanical: bool,
    pub include_prototype: bool,
    pub include_non_runnable: bool,
    pub include_uncatalogued: bool,
    pub only_available: bool,
}

pub fn filter_roms(roms: &[RomEntry], options: &FilterOptions) -> Vec<RomEntry> {
    roms.iter()
        .filter(|rom| matches_options(rom, options))
        .cloned()
        .collect()
}

fn matches_options(rom: &RomEntry, options: &FilterOptions) -> bool {
    if !options.include_non_runnable
        && !rom.metadata.flags.runnable
        && !(options.include_mechanical && rom.metadata.flags.mechanical)
        && !(options.include_uncatalogued && !rom.catalogued)
    {
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

    if !options.regions.is_empty() && !options.regions.contains(&rom.metadata.region) {
        return false;
    }

    if options.regions.is_empty()
        && let Some(region) = &options.region
        && &rom.metadata.region != region
    {
        return false;
    }

    if !matches_any(&options.names, |pattern| glob_matches(pattern, &rom.name)) {
        return false;
    }

    if !matches_any(&options.manufacturers, |expected| {
        rom.manufacturer
            .as_deref()
            .is_some_and(|actual| contains_ci(actual, expected))
    }) {
        return false;
    }

    if options.year_from.is_some() || options.year_to.is_some() {
        let Some(year) = rom
            .year
            .as_deref()
            .and_then(|year| year.parse::<u16>().ok())
        else {
            return false;
        };
        if options.year_from.is_some_and(|start| year < start)
            || options.year_to.is_some_and(|end| year > end)
        {
            return false;
        }
    }

    if !matches_any(&options.categories, |expected| {
        rom.metadata
            .genre
            .as_ref()
            .is_some_and(|genre| contains_ci(&genre.category, expected))
    }) {
        return false;
    }

    if !matches_any(&options.subcategories, |expected| {
        rom.metadata.genre.as_ref().is_some_and(|genre| {
            genre
                .subcategory
                .as_deref()
                .is_some_and(|actual| contains_ci(actual, expected))
        })
    }) {
        return false;
    }

    if !matches_any(&options.genres, |expected| {
        rom.metadata.genre.as_ref().is_some_and(|genre| {
            contains_ci(&genre.category, expected)
                || genre
                    .subcategory
                    .as_deref()
                    .is_some_and(|actual| contains_ci(actual, expected))
        })
    }) {
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

fn matches_any(values: &[String], predicate: impl Fn(&str) -> bool) -> bool {
    values.is_empty() || values.iter().any(|value| predicate(value))
}

fn contains_ci(actual: &str, expected: &str) -> bool {
    actual
        .to_ascii_lowercase()
        .contains(&expected.to_ascii_lowercase())
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    fn matches(pattern: &[u8], value: &[u8]) -> bool {
        match pattern {
            [] => value.is_empty(),
            [b'*', rest @ ..] => {
                matches(rest, value) || (!value.is_empty() && matches(pattern, &value[1..]))
            }
            [b'?', rest @ ..] => !value.is_empty() && matches(rest, &value[1..]),
            [first, rest @ ..] => {
                value
                    .first()
                    .is_some_and(|actual| first.eq_ignore_ascii_case(actual))
                    && matches(rest, &value[1..])
            }
        }
    }

    matches(pattern.as_bytes(), value.as_bytes())
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
            catalogued: true,
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
            catalogued: true,
        }];

        assert!(filter_roms(&roms, &FilterOptions::default()).is_empty());
    }

    #[test]
    fn includes_non_runnable_mechanical_entries_when_requested() {
        let roms = vec![RomEntry {
            name: "mechanical".to_string(),
            description: None,
            year: None,
            manufacturer: None,
            metadata: RomMetadata {
                flags: Flags {
                    mechanical: true,
                    runnable: false,
                    ..Flags::default()
                },
                ..RomMetadata::default()
            },
            rom_path: None,
            catalogued: true,
        }];

        assert_eq!(
            filter_roms(
                &roms,
                &FilterOptions {
                    include_mechanical: true,
                    ..FilterOptions::default()
                }
            ),
            roms
        );
    }

    #[test]
    fn same_kind_selectors_or_and_different_kinds_and() {
        let mut pacman = RomEntry::new("pacman");
        pacman.year = Some("1980".to_string());
        pacman.manufacturer = Some("Namco".to_string());
        let mut galaga = RomEntry::new("galaga");
        galaga.year = Some("1981".to_string());
        galaga.manufacturer = Some("Namco".to_string());

        let result = filter_roms(
            &[pacman, galaga.clone()],
            &FilterOptions {
                names: vec!["pac*".to_string(), "gal*".to_string()],
                manufacturers: vec!["nam".to_string()],
                year_from: Some(1981),
                year_to: Some(1981),
                ..FilterOptions::default()
            },
        );

        assert_eq!(result, vec![galaga]);
    }
}
