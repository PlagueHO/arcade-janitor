use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
};

use serde_json::{Value, json};
use tempfile::TempDir;

pub const MAME_XML: &str = r#"<mame>
<machine name="pacman"><description>Pac-Man (USA)</description><year>1980</year><manufacturer>Namco</manufacturer></machine>
<machine name="galaga"><description>Galaga (Japan)</description><year>1981</year><manufacturer>Namco</manufacturer></machine>
<machine name="missing"><description>Missing (World)</description><year>1982</year><manufacturer>Example</manufacturer></machine>
</mame>"#;
pub const CATVER: &str =
    "[Category]\npacman=Maze / Chase\ngalaga=Shooter / Flying Vertical\nmissing=Maze / Chase\n";
pub const PACMAN_BYTES: &[u8] = b"pacman archive\x00\x01";
pub const GALAGA_BYTES: &[u8] = b"galaga archive\x02\x03";
pub const UNKNOWN_BYTES: &[u8] = b"unknown archive\x04\x05";
pub const NOTE_BYTES: &[u8] = b"do not move or delete this file";

pub struct FlowFixture {
    pub directory: TempDir,
    pub roms: PathBuf,
    pub mame_xml: PathBuf,
    pub catver: PathBuf,
}

impl FlowFixture {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let roms = directory.path().join("roms");
        let mame_xml = directory.path().join("mame.xml");
        let catver = directory.path().join("catver.ini");
        fs::create_dir(&roms).unwrap();
        fs::write(&mame_xml, MAME_XML).unwrap();
        fs::write(&catver, CATVER).unwrap();
        fs::write(roms.join("pacman.zip"), PACMAN_BYTES).unwrap();
        fs::write(roms.join("galaga.7z"), GALAGA_BYTES).unwrap();
        fs::write(roms.join("unknown.zip"), UNKNOWN_BYTES).unwrap();
        fs::write(roms.join("notes.txt"), NOTE_BYTES).unwrap();
        Self {
            directory,
            roms,
            mame_xml,
            catver,
        }
    }

    pub fn assert_unrelated_files(&self) {
        assert_eq!(fs::read(self.roms.join("galaga.7z")).unwrap(), GALAGA_BYTES);
        assert_eq!(
            fs::read(self.roms.join("unknown.zip")).unwrap(),
            UNKNOWN_BYTES
        );
        assert_eq!(fs::read(self.roms.join("notes.txt")).unwrap(), NOTE_BYTES);
    }
}

pub fn environment(root: &Path) -> Vec<(OsString, OsString)> {
    for directory in ["home", "cache", "config", "tmp"] {
        fs::create_dir_all(root.join(directory)).unwrap();
    }
    [
        ("ARCADEJANITOR_CACHE_DIR", root.join("cache")),
        ("HOME", root.join("home")),
        ("USERPROFILE", root.join("home")),
        ("LOCALAPPDATA", root.join("cache")),
        ("APPDATA", root.join("config")),
        ("XDG_CACHE_HOME", root.join("cache")),
        ("XDG_CONFIG_HOME", root.join("config")),
        ("TMPDIR", root.join("tmp")),
        ("TMP", root.join("tmp")),
        ("TEMP", root.join("tmp")),
    ]
    .into_iter()
    .map(|(name, path)| (OsString::from(name), path.into_os_string()))
    .collect()
}

pub fn inherited_application_settings() -> Vec<OsString> {
    std::env::vars_os()
        .map(|(name, _)| name)
        .filter(|name| is_application_setting(name))
        .collect()
}

fn is_application_setting(name: &OsStr) -> bool {
    let name = name.to_string_lossy().to_ascii_uppercase();
    name.starts_with("ARCADEJANITOR_") || name.starts_with("FAKE_MAME_")
}

#[test]
fn application_setting_filter_handles_case_insensitive_windows_names() {
    for name in [
        "ARCADEJANITOR_MAME_XML",
        "arcadejanitor_mame_executable",
        "ArcadeJanitor_Mcp_Token",
        "FAKE_MAME_XML",
        "fake_mame_mode",
    ] {
        assert!(is_application_setting(OsStr::new(name)), "{name}");
    }
    for name in ["PATH", "SystemRoot", "CARGO_HOME", "RUSTUP_HOME"] {
        assert!(!is_application_setting(OsStr::new(name)), "{name}");
    }
}

pub fn names(rows: &Value) -> Vec<String> {
    rows.as_array()
        .unwrap()
        .iter()
        .map(|row| row["name"].as_str().unwrap().to_string())
        .collect()
}

pub fn expected_collection() -> Value {
    json!([
        {"name": "galaga", "description": "Galaga (Japan)", "year": "1981",
         "manufacturer": "Namco", "category": "Shooter", "subcategory": "Flying Vertical",
         "available": true, "catalogued": true},
        {"name": "missing", "description": "Missing (World)", "year": "1982",
         "manufacturer": "Example", "category": "Maze", "subcategory": "Chase",
         "available": false, "catalogued": true},
        {"name": "pacman", "description": "Pac-Man (USA)", "year": "1980",
         "manufacturer": "Namco", "category": "Maze", "subcategory": "Chase",
         "available": true, "catalogued": true},
        {"name": "unknown", "description": null, "year": null,
         "manufacturer": null, "category": null, "subcategory": null,
         "available": true, "catalogued": false}
    ])
}

pub fn normalize_roms(rows: &Value, mcp: bool) -> Value {
    let mut rows: Vec<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let (category, subcategory, available) = if mcp {
                (
                    row["metadata"]["genre"]["category"].clone(),
                    row["metadata"]["genre"]["subcategory"].clone(),
                    !row["rom_path"].is_null(),
                )
            } else {
                (
                    row["category"].clone(),
                    row["subcategory"].clone(),
                    row["available"].as_bool().unwrap(),
                )
            };
            json!({
                "name": row["name"], "description": row["description"], "year": row["year"],
                "manufacturer": row["manufacturer"], "category": category,
                "subcategory": subcategory, "available": available, "catalogued": row["catalogued"]
            })
        })
        .collect();
    rows.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    json!(rows)
}

pub fn expected_categories() -> Value {
    json!([{"category": "Maze", "count": 2}, {"category": "Shooter", "count": 1}])
}

pub fn normalize_categories(rows: &Value, mcp: bool) -> Value {
    json!(
        rows.as_array()
            .unwrap()
            .iter()
            .map(|row| json!({
                "category": row["category"],
                "count": row[if mcp { "rom_count" } else { "entries" }]
            }))
            .collect::<Vec<_>>()
    )
}

pub fn assert_initial_scan(rows: &Value, mcp: bool) {
    assert_eq!(normalize_roms(rows, mcp), expected_collection());
}

pub fn assert_rescan(rows: &Value, mcp: bool) {
    let mut expected = expected_collection();
    expected[2]["available"] = json!(false);
    assert_eq!(normalize_roms(rows, mcp), expected);
}
