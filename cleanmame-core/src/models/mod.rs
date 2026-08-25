use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RomEntry {
    pub name: String,
    pub description: Option<String>,
    pub year: Option<String>,
    pub manufacturer: Option<String>,
    pub metadata: RomMetadata,
    pub rom_path: Option<PathBuf>,
}

impl RomEntry {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            year: None,
            manufacturer: None,
            metadata: RomMetadata::default(),
            rom_path: None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RomMetadata {
    pub genre: Option<Genre>,
    pub region: Region,
    pub flags: Flags,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Genre {
    pub category: String,
    pub subcategory: Option<String>,
}

impl Genre {
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }

        let mut parts = value.splitn(2, '/').map(str::trim);
        let category = parts.next()?.to_string();
        let subcategory = parts
            .next()
            .filter(|part| !part.is_empty())
            .map(ToOwned::to_owned);

        Some(Self {
            category,
            subcategory,
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Region {
    Usa,
    Japan,
    Europe,
    World,
    Asia,
    Other(String),
    #[default]
    Unknown,
}

impl Region {
    pub fn infer(name: &str, description: Option<&str>) -> Self {
        let text = format!(
            "{} {}",
            name.to_ascii_lowercase(),
            description.unwrap_or_default().to_ascii_lowercase()
        );

        if text.contains("japan") || text.contains("(jp") {
            Self::Japan
        } else if text.contains("europe") || text.contains("(eu") {
            Self::Europe
        } else if text.contains("usa") || text.contains("u.s") || text.contains("(us") {
            Self::Usa
        } else if text.contains("asia") {
            Self::Asia
        } else if text.contains("world") {
            Self::World
        } else {
            Self::Unknown
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Flags {
    pub mechanical: bool,
    pub mature: bool,
    pub prototype: bool,
    pub runnable: bool,
}
