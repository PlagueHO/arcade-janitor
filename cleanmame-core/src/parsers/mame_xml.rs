use std::{fs, path::Path};

use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};

use crate::{CleanMameError, Result, errors::io_error, models::RomEntry};

pub fn parse_mame_xml_file(path: impl AsRef<Path>) -> Result<Vec<RomEntry>> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|source| io_error(path, source))?;
    parse_mame_xml_str(&content)
}

pub fn parse_mame_xml_str(content: &str) -> Result<Vec<RomEntry>> {
    let mut reader = Reader::from_str(content);

    let mut entries = Vec::new();
    let mut current: Option<RomEntry> = None;
    let mut current_field: Option<Field> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) if matches!(element.name().as_ref(), "machine" | "game") => {
                current = parse_machine(&element)?;
            }
            Ok(Event::Empty(element)) if matches!(element.name().as_ref(), "machine" | "game") => {
                if let Some(entry) = parse_machine(&element)? {
                    entries.push(finalize_entry(entry));
                }
            }
            Ok(Event::Start(element)) => {
                current_field = match element.name().as_ref() {
                    "description" => Some(Field::Description),
                    "year" => Some(Field::Year),
                    "manufacturer" => Some(Field::Manufacturer),
                    _ => None,
                };
            }
            Ok(Event::Text(text)) => {
                if let (Some(entry), Some(field)) = (current.as_mut(), current_field) {
                    let value = quick_xml::escape::unescape(text.as_ref())
                        .map_err(|error| CleanMameError::Xml(error.to_string()))?
                        .into_owned();
                    append_field(entry, field, &value);
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if let (Some(entry), Some(field)) = (current.as_mut(), current_field) {
                    let value = if let Some(character) = reference
                        .resolve_char_ref()
                        .map_err(|error| CleanMameError::Xml(error.to_string()))?
                    {
                        character.to_string()
                    } else {
                        quick_xml::escape::resolve_predefined_entity(reference.as_ref())
                            .ok_or_else(|| {
                                CleanMameError::Xml(format!(
                                    "unrecognized entity '{}'",
                                    reference.as_ref()
                                ))
                            })?
                            .to_string()
                    };
                    append_field(entry, field, &value);
                }
            }
            Ok(Event::End(element)) if matches!(element.name().as_ref(), "machine" | "game") => {
                if let Some(entry) = current.take() {
                    entries.push(finalize_entry(entry));
                }
            }
            Ok(Event::End(element))
                if matches!(
                    element.name().as_ref(),
                    "description" | "year" | "manufacturer"
                ) =>
            {
                current_field = None;
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(CleanMameError::Xml(error.to_string())),
            _ => {}
        }
    }

    fn append_field(entry: &mut RomEntry, field: Field, value: &str) {
        let target = match field {
            Field::Description => &mut entry.description,
            Field::Year => &mut entry.year,
            Field::Manufacturer => &mut entry.manufacturer,
        };
        target.get_or_insert_default().push_str(value);
    }

    Ok(entries)
}

fn parse_machine(element: &BytesStart<'_>) -> Result<Option<RomEntry>> {
    let mut name = None;
    let mut runnable = true;
    let mut excluded = false;
    let mut mechanical = false;

    for attr in element.attributes() {
        let attr = attr.map_err(|error| CleanMameError::Xml(error.to_string()))?;
        let value = attr
            .normalized_value(Default::default())
            .map_err(|error| CleanMameError::Xml(error.to_string()))?
            .into_owned();
        match attr.key.as_ref() {
            "name" => name = Some(value),
            "runnable" => runnable = value != "no",
            "ismechanical" => mechanical = value == "yes",
            "isdevice" | "isbios" if value == "yes" => excluded = true,
            _ => {}
        }
    }

    Ok(name.map(|name| {
        let mut entry = RomEntry::new(name);
        entry.metadata.flags.runnable = runnable && !excluded;
        entry.metadata.flags.mechanical = mechanical;
        entry
    }))
}

fn finalize_entry(mut entry: RomEntry) -> RomEntry {
    entry.metadata.region = crate::models::Region::infer(&entry.name, entry.description.as_deref());
    entry.metadata.flags.mature |= has_mature_marker(&entry.name)
        || entry.description.as_deref().is_some_and(has_mature_marker);
    entry.metadata.flags.prototype |= has_prototype_marker(&entry.name)
        || entry
            .description
            .as_deref()
            .is_some_and(has_prototype_marker);
    entry
}

fn has_mature_marker(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.contains("mature") || value.contains("adult")
}

fn has_prototype_marker(value: &str) -> bool {
    value.to_ascii_lowercase().contains("prototype")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Field {
    Description,
    Year,
    Manufacturer,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_machine_metadata() {
        let xml = r#"<mame><machine name="pacman&amp;"><description>Pac-Man &amp; Friends (USA)</description><year>1980</year><manufacturer>Namco</manufacturer><driver status="preliminary"/></machine></mame>"#;
        let roms = parse_mame_xml_str(xml).unwrap();

        assert_eq!(roms.len(), 1);
        assert_eq!(roms[0].name, "pacman&");
        assert_eq!(
            roms[0].description.as_deref(),
            Some("Pac-Man & Friends (USA)")
        );
        assert!(roms[0].metadata.flags.runnable);
        assert!(!roms[0].metadata.flags.prototype);
    }

    #[test]
    fn excludes_devices_regardless_of_attribute_order() {
        let xml = r#"<mame><machine name="device-first" isdevice="yes" runnable="yes"/><machine name="runnable-first" runnable="yes" isbios="yes"/></mame>"#;
        let roms = parse_mame_xml_str(xml).unwrap();

        assert!(roms.iter().all(|rom| !rom.metadata.flags.runnable));
    }

    #[test]
    fn identifies_prototype_descriptions() {
        let xml = r#"<mame><machine name="game"><description>Game (Prototype)</description></machine></mame>"#;
        let roms = parse_mame_xml_str(xml).unwrap();

        assert!(roms[0].metadata.flags.prototype);
    }
}
