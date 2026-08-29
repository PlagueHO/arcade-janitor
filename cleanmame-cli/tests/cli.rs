use std::path::Path;

use assert_cmd::{Command, cargo::cargo_bin};
use predicates::prelude::*;
use tempfile::TempDir;

fn command() -> Command {
    Command::new(cargo_bin!("cleanmame"))
}

fn fixture() -> TempDir {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("mame.xml"),
        r#"<mame>
<machine name="pacman"><description>Pac-Man (USA)</description><year>1980</year><manufacturer>Namco</manufacturer></machine>
<machine name="galaga"><description>Galaga (Japan)</description><year>1981</year><manufacturer>Namco</manufacturer></machine>
<machine name="prototype"><description>Prototype</description><year>1982</year><manufacturer>Example</manufacturer></machine>
</mame>"#,
    )
    .unwrap();
    std::fs::write(
        directory.path().join("catver.ini"),
        "[Category]\npacman=Maze / Chase\ngalaga=Shooter / Flying Vertical\nprototype=Prototype\n",
    )
    .unwrap();
    std::fs::create_dir(directory.path().join("roms")).unwrap();
    std::fs::write(directory.path().join("roms").join("pacman.zip"), b"rom").unwrap();
    std::fs::write(directory.path().join("roms").join("unknown.7z"), b"rom").unwrap();
    directory
}

fn source_args(directory: &Path) -> [String; 4] {
    [
        "--mame-xml".to_string(),
        directory.join("mame.xml").display().to_string(),
        "--catver".to_string(),
        directory.join("catver.ini").display().to_string(),
    ]
}

#[test]
fn top_level_help_exposes_resource_groups() {
    let output = command()
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8(output).unwrap();

    assert!(help.contains("Inspect and manage MAME ROM collections"));
    for resource in ["rom", "catalog", "category", "source", "completions"] {
        assert!(help.contains(resource), "missing resource {resource}");
    }
    for removed in [
        "\n  scan ",
        "\n  filter ",
        "\n  move ",
        "\n  delete ",
        "\n  report ",
        "\n  mame ",
        "\n  catver ",
    ] {
        assert!(
            !help.contains(removed),
            "legacy command remains: {removed:?}"
        );
    }
}

#[test]
fn nested_help_is_complete_and_consistent() {
    for (group, commands) in [
        (
            "rom",
            &["list", "show", "move", "delete", "stats", "audit"][..],
        ),
        ("catalog", &["list", "show"][..]),
        ("category", &["list", "show"][..]),
        ("source", &["list", "refresh", "clear"][..]),
    ] {
        let output = command()
            .args([group, "--help"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let help = String::from_utf8(output).unwrap();
        for subcommand in commands {
            assert!(
                help.contains(subcommand),
                "missing {group} {subcommand} from help"
            );
        }
    }
}

#[test]
fn legacy_commands_are_rejected() {
    for legacy in [
        "scan", "filter", "move", "delete", "report", "mame", "catver",
    ] {
        command().arg(legacy).assert().failure();
    }
}

#[test]
fn global_source_and_output_options_work_after_subcommands() {
    let directory = fixture();
    let mut arguments = vec![
        "catalog".to_string(),
        "show".to_string(),
        "pacman".to_string(),
    ];
    arguments.extend(source_args(directory.path()));
    arguments.extend(["--output".to_string(), "json".to_string()]);

    command()
        .args(arguments)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""name": "pacman""#))
        .stdout(predicate::str::contains(r#""category": "Maze""#));
}

#[test]
fn catalog_list_applies_repeatable_or_and_cross_field_and_selectors() {
    let directory = fixture();
    let mut arguments = vec![
        "catalog".to_string(),
        "list".to_string(),
        "--name".to_string(),
        "pac*".to_string(),
        "--name".to_string(),
        "gal*".to_string(),
        "--year".to_string(),
        "1981".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];
    arguments.extend(source_args(directory.path()));

    command()
        .args(arguments)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""name": "galaga""#))
        .stdout(predicate::str::contains(r#""name": "pacman""#).not());
}

#[test]
fn rom_list_has_explicit_availability_populations() {
    let directory = fixture();
    let mut available = vec![
        "rom".to_string(),
        "list".to_string(),
        directory.path().join("roms").display().to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];
    available.extend(source_args(directory.path()));

    command()
        .args(&available)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""name": "pacman""#))
        .stdout(predicate::str::contains(r#""name": "unknown""#).not());

    let mut unmatched = available;
    unmatched.extend(["--status".to_string(), "unmatched".to_string()]);
    command()
        .args(unmatched)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""name": "unknown""#));
}

#[test]
fn rom_stats_filters_categories_and_can_show_missing_roms() {
    let directory = fixture();
    let mut arguments = vec![
        "rom".to_string(),
        "stats".to_string(),
        directory.path().join("roms").display().to_string(),
        "--category".to_string(),
        "Shooter".to_string(),
        "--category".to_string(),
        "Maze".to_string(),
        "--subcategory".to_string(),
        "Flying Vertical".to_string(),
        "--subcategory".to_string(),
        "Chase".to_string(),
        "--show-missing".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];
    arguments.extend(source_args(directory.path()));

    command()
        .args(arguments)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""missing_roms""#))
        .stdout(predicate::str::contains(r#""galaga""#))
        .stdout(predicate::str::contains(r#""missing": 1"#));
}

#[test]
fn mutations_require_selection_and_preview_by_default() {
    let directory = fixture();
    let rom_dir = directory.path().join("roms");
    let destination = directory.path().join("moved");
    let mut unqualified = vec![
        "rom".to_string(),
        "move".to_string(),
        rom_dir.display().to_string(),
        destination.display().to_string(),
    ];
    unqualified.extend(source_args(directory.path()));
    command()
        .args(&unqualified)
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires at least one selector"));

    let mut preview = unqualified;
    preview.extend([
        "--name".to_string(),
        "pacman".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ]);
    command()
        .args(preview)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""state": "preview""#));

    assert!(rom_dir.join("pacman.zip").is_file());
    assert!(!destination.exists());

    let mut inclusion_only = vec![
        "rom".to_string(),
        "delete".to_string(),
        rom_dir.display().to_string(),
        "--include-mature".to_string(),
    ];
    inclusion_only.extend(source_args(directory.path()));
    command()
        .args(inclusion_only)
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires at least one selector"));

    let mut empty_selector = vec![
        "rom".to_string(),
        "delete".to_string(),
        rom_dir.display().to_string(),
        "--category=".to_string(),
        "--execute".to_string(),
    ];
    empty_selector.extend(source_args(directory.path()));
    command()
        .args(empty_selector)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "selector values must not be empty",
        ));
}

#[test]
fn mutation_executes_only_with_execute() {
    let directory = fixture();
    let rom_dir = directory.path().join("roms");
    let destination = directory.path().join("moved");
    let mut arguments = vec![
        "rom".to_string(),
        "move".to_string(),
        rom_dir.display().to_string(),
        destination.display().to_string(),
        "--name".to_string(),
        "pacman".to_string(),
        "--execute".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];
    arguments.extend(source_args(directory.path()));

    command()
        .args(arguments)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""state": "executed""#));

    assert!(!rom_dir.join("pacman.zip").exists());
    assert!(destination.join("pacman.zip").is_file());
}

#[test]
fn tsv_output_is_header_controllable_and_status_stays_off_stdout() {
    let directory = fixture();
    let mut arguments = vec![
        "catalog".to_string(),
        "show".to_string(),
        "pacman".to_string(),
        "--output".to_string(),
        "tsv".to_string(),
        "--no-header".to_string(),
    ];
    arguments.extend(source_args(directory.path()));

    command()
        .args(arguments)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("pacman\t"))
        .stdout(predicate::str::contains("Resolving metadata").not());
}

#[test]
fn category_commands_replace_raw_catver_views() {
    let directory = fixture();
    let mut list = vec![
        "category".to_string(),
        "list".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];
    list.extend(source_args(directory.path()));
    command()
        .args(list)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""category": "Maze""#));

    let mut show = vec![
        "category".to_string(),
        "show".to_string(),
        "Shooter".to_string(),
        "--subcategory".to_string(),
        "vertical".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];
    show.extend(source_args(directory.path()));
    command()
        .args(show)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""name": "galaga""#));
}

#[test]
fn completions_generates_a_shell_script() {
    command()
        .args(["completions", "powershell"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Register-ArgumentCompleter"));
}

#[test]
fn source_clear_previews_by_default() {
    command()
        .args(["source", "clear", "mame", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""action": "clear""#))
        .stdout(predicate::str::contains(r#""state": "preview""#));

    command()
        .args(["source", "clear", "--execute"])
        .assert()
        .failure();
}

#[test]
fn stats_and_explicit_mutations_include_unmatched_archives() {
    let directory = fixture();
    let rom_dir = directory.path().join("roms");
    let mut stats = vec![
        "rom".to_string(),
        "stats".to_string(),
        rom_dir.display().to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];
    stats.extend(source_args(directory.path()));
    command()
        .args(stats)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""unmatched": 1"#));

    let mut delete = vec![
        "rom".to_string(),
        "delete".to_string(),
        rom_dir.display().to_string(),
        "--name".to_string(),
        "unknown".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];
    delete.extend(source_args(directory.path()));
    command()
        .args(delete)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""name": "unknown""#))
        .stdout(predicate::str::contains(r#""state": "preview""#));
}
