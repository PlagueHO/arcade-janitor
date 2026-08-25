use std::{fs, path::Path};

use quick_xml::{Reader, events::Event};

use crate::{CleanMameError, Result, errors::io_error, models::RomEntry};

pub fn parse_mame_xml_file(path: impl AsRef<Path>) -> Result<Vec<RomEntry>> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|source| io_error(path, source))?;
    parse_mame_xml_str(&content)
}

pub fn parse_mame_xml_str(content: &str) -> Result<Vec<RomEntry>> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    let mut entries = Vec::new();
    let mut current: Option<RomEntry> = None;
    let mut current_field: Option<Field> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) if matches!(element.name().as_ref(), "machine" | "game") => {
                let mut name = None;
                let mut runnable = true;
                let mut mechanical = false;

                for attr in element.attributes().flatten() {
                    let value = attr.value.as_ref().to_string();
                    match attr.key.as_ref() {
                        "name" => name = Some(value),
                        "runnable" => runnable = value != "no",
                        "ismechanical" => mechanical = value == "yes",
                        "isdevice" | "isbios" if value == "yes" => runnable = false,
                        _ => {}
                    }
                }

                if let Some(name) = name {
                    let mut entry = RomEntry::new(name);
                    entry.metadata.flags.runnable = runnable;
                    entry.metadata.flags.mechanical = mechanical;
                    current = Some(entry);
                }
            }
            Ok(Event::Start(element)) => {
                current_field = match element.name().as_ref() {
                    "description" => Some(Field::Description),
                    "year" => Some(Field::Year),
                    "manufacturer" => Some(Field::Manufacturer),
                    "driver" => Some(Field::Driver),
                    _ => None,
                };

                if current_field == Some(Field::Driver) {
                    if let Some(entry) = current.as_mut() {
                        for attr in element.attributes().flatten() {
                            let value = attr.value.as_ref().to_ascii_lowercase();
                            if attr.key.as_ref() == "status" && value == "preliminary" {
                                entry.metadata.flags.prototype = true;
                            }
                            if attr.key.as_ref() == "emulation" && value == "preliminary" {
                                entry.metadata.flags.prototype = true;
                            }
                        }
                    }
                    current_field = None;
                }
            }
            Ok(Event::Text(text)) => {
                if let (Some(entry), Some(field)) = (current.as_mut(), current_field) {
                    let value = text.as_ref().to_string();
                    match field {
                        Field::Description => entry.description = Some(value),
                        Field::Year => entry.year = Some(value),
                        Field::Manufacturer => entry.manufacturer = Some(value),
                        Field::Driver => {}
                    }
                }
            }
            Ok(Event::End(element)) if matches!(element.name().as_ref(), "machine" | "game") => {
                if let Some(mut entry) = current.take() {
                    entry.metadata.region =
                        crate::models::Region::infer(&entry.name, entry.description.as_deref());
                    entry.metadata.flags.mature |= has_mature_marker(&entry.name)
                        || entry.description.as_deref().is_some_and(has_mature_marker);
                    entries.push(entry);
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(CleanMameError::Xml(error.to_string())),
            _ => {}
        }
    }

    Ok(entries)
}

fn has_mature_marker(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.contains("mature") || value.contains("adult") || value.contains("mahjong")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Field {
    Description,
    Year,
    Manufacturer,
    Driver,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_machine_metadata() {
        let xml = r#"<mame><machine name="pacman"><description>Pac-Man (USA)</description><year>1980</year><manufacturer>Namco</manufacturer></machine></mame>"#;
        let roms = parse_mame_xml_str(xml).unwrap();

        assert_eq!(roms.len(), 1);
        assert_eq!(roms[0].name, "pacman");
        assert_eq!(roms[0].description.as_deref(), Some("Pac-Man (USA)"));
        assert!(roms[0].metadata.flags.runnable);
    }
}
