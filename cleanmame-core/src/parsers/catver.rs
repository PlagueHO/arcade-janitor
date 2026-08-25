use std::{collections::HashMap, fs, path::Path};

use crate::{CleanMameError, Result, errors::io_error, models::Genre};

pub fn parse_catver_file(path: impl AsRef<Path>) -> Result<HashMap<String, Genre>> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|source| io_error(path, source))?;
    parse_catver_str(&content)
}

pub fn parse_catver_str(content: &str) -> Result<HashMap<String, Genre>> {
    let parsed = ini::macro_safe_read(content).map_err(CleanMameError::Ini)?;
    let mut genres = HashMap::new();

    if let Some(category) = parsed.get("category").or_else(|| parsed.get("Category")) {
        for (rom, value) in category {
            if let Some(genre) = value.as_deref().and_then(Genre::parse) {
                genres.insert(rom.to_string(), genre);
            }
        }
    }

    Ok(genres)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_category_section() {
        let genres = parse_catver_str("[Category]\npacman=Maze / Chase\n").unwrap();

        assert_eq!(genres["pacman"].category, "Maze");
        assert_eq!(genres["pacman"].subcategory.as_deref(), Some("Chase"));
    }
}
