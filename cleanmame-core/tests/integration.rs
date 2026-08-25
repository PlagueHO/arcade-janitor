use std::{fs, path::PathBuf};

use cleanmame_core::operations::{
    filter::{FilterOptions, filter_roms},
    query::scan_rom_folder,
    report::generate_report,
};

#[test]
fn scan_filter_and_report_rom_folder() {
    let root = unique_temp_dir();
    let rom_dir = root.join("roms");
    fs::create_dir_all(&rom_dir).unwrap();
    fs::write(rom_dir.join("pacman.zip"), "").unwrap();

    let mame_xml = root.join("mame.xml");
    fs::write(
        &mame_xml,
        r#"<mame><machine name="pacman"><description>Pac-Man (USA)</description><year>1980</year><manufacturer>Namco</manufacturer></machine></mame>"#,
    )
    .unwrap();

    let catver = root.join("catver.ini");
    fs::write(&catver, "[Category]\npacman=Maze / Chase\n").unwrap();

    let roms = scan_rom_folder(&rom_dir, &mame_xml, Some(&catver)).unwrap();
    let filtered = filter_roms(
        &roms,
        &FilterOptions {
            genre_contains: Some("maze".to_string()),
            only_available: true,
            ..FilterOptions::default()
        },
    );
    let report = generate_report(&roms);

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "pacman");
    assert_eq!(report.total, 1);
    assert_eq!(report.available, 1);

    fs::remove_dir_all(root).unwrap();
}

fn unique_temp_dir() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "cleanmame-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    path
}
