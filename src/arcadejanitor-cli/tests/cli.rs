use std::{
    fs,
    ops::{Deref, DerefMut},
    path::Path,
    process::Output,
    time::Duration,
};

use assert_cmd::{Command, cargo::cargo_bin};
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

#[path = "../../../tests/support/mod.rs"]
mod support;

use support::{FlowFixture, PACMAN_BYTES, names};

struct IsolatedCommand {
    command: Command,
    _directory: TempDir,
}

impl Deref for IsolatedCommand {
    type Target = Command;

    fn deref(&self) -> &Self::Target {
        &self.command
    }
}

impl DerefMut for IsolatedCommand {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.command
    }
}

fn command_in(root: &Path) -> Command {
    let mut command = Command::new(cargo_bin!("arcadejanitor"));
    for name in support::inherited_application_settings() {
        command.env_remove(name);
    }
    command
        .envs(support::environment(root))
        .current_dir(root)
        .timeout(Duration::from_secs(15));
    command
}

fn command() -> IsolatedCommand {
    let directory = tempfile::tempdir().unwrap();
    IsolatedCommand {
        command: command_in(directory.path()),
        _directory: directory,
    }
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

fn large_fixture() -> (&'static Path, &'static Path) {
    (
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/mame-100.xml"
        )),
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/catver-100.ini"
        )),
    )
}

#[test]
fn scans_committed_100_entry_fixture_with_temporary_roms() {
    let directory = tempfile::tempdir().unwrap();
    let roms = directory.path().join("roms");
    std::fs::create_dir(&roms).unwrap();
    for name in [
        "game001.zip",
        "game050.7z",
        "game100.zip",
        "not-in-catalog.zip",
    ] {
        std::fs::write(roms.join(name), b"fixture rom").unwrap();
    }
    let (mame_xml, catver) = large_fixture();

    let catalog_output = command()
        .args([
            "catalog",
            "list",
            "--output",
            "json",
            "--mame-xml",
            mame_xml.to_str().unwrap(),
            "--catver",
            catver.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(catalog_output.status.success());
    let catalog: Value = serde_json::from_slice(&catalog_output.stdout).unwrap();
    assert_eq!(catalog.as_array().unwrap().len(), 100);

    let output = command()
        .args([
            "rom",
            "list",
            roms.to_str().unwrap(),
            "--output",
            "json",
            "--mame-xml",
            mame_xml.to_str().unwrap(),
            "--catver",
            catver.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let available = output_json(&output, 0);
    assert_eq!(names(&available), ["game001", "game050", "game100"]);
    for row in available.as_array().unwrap() {
        assert_eq!(row["available"], true);
        assert_eq!(row["catalogued"], true);
    }
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
    for resource in ["rom", "catalog", "category", "source", "mcp", "completions"] {
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
        ("mcp", &["start", "install"][..]),
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
fn mcp_install_help_displays_supported_systems() {
    command()
        .args(["mcp", "install", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "--system <vscode|vscode-insiders|copilot-cli|claude-code> [ROM_DIR]",
        ));
}

#[test]
fn only_rom_directories_are_positional_arguments() {
    for (arguments, required_option) in [
        (&["rom", "show", "--help"][..], "--name <NAME>"),
        (
            &["rom", "move", "--help"][..],
            "--destination <DESTINATION>",
        ),
        (&["catalog", "show", "--help"][..], "--name <NAME>"),
        (&["category", "show", "--help"][..], "--category <CATEGORY>"),
        (&["source", "refresh", "--help"][..], "--target <TARGET>"),
        (&["source", "clear", "--help"][..], "--target <TARGET>"),
        (&["completions", "--help"][..], "--shell <SHELL>"),
    ] {
        let help = command()
            .args(arguments)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let help = String::from_utf8(help).unwrap().replace("\r\n", "\n");

        assert!(
            help.contains(required_option),
            "missing named option {required_option} in {arguments:?}"
        );
        assert!(
            !help.contains(&format!("Arguments:\n  {required_option}")),
            "option is still positional in {arguments:?}"
        );
    }
}

#[test]
fn mcp_install_validates_transport_specific_arguments() {
    command()
        .args([
            "mcp",
            "install",
            "--system",
            "vscode",
            "--transport",
            "stdio",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "ROM_DIR is required when installing with --transport stdio",
        ));

    command()
        .args([
            "mcp",
            "install",
            "--system",
            "vscode",
            "--transport",
            "http",
            "--start-now",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "ROM_DIR is required when using --start-now",
        ));

    command()
        .args([
            "mcp",
            "install",
            "--system",
            "vscode",
            "roms",
            "--transport",
            "stdio",
            "--start-now",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "--start-now is only valid with --transport http",
        ));

    command()
        .args([
            "mcp",
            "install",
            "--system",
            "vscode",
            "roms",
            "--transport",
            "stdio",
            "--url",
            "http://localhost:3000/mcp",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "--url is only valid with --transport http",
        ));
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
        "--name".to_string(),
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
        .stdout(predicate::str::contains(r#""available": 0"#))
        .stdout(predicate::str::contains(r#""missing": 1"#));
}

#[test]
fn rom_stats_shows_detailed_missing_and_unmatched_roms_only_when_requested() {
    let directory = fixture();
    let rom_dir = directory.path().join("roms");
    let mut default_arguments = vec![
        "rom".to_string(),
        "stats".to_string(),
        rom_dir.display().to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];
    default_arguments.extend(source_args(directory.path()));

    command()
        .args(&default_arguments)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""missing": 1"#))
        .stdout(predicate::str::contains(r#""unmatched": 1"#))
        .stdout(predicate::str::contains(r#""missing_roms""#).not())
        .stdout(predicate::str::contains(r#""unmatched_roms""#).not());

    let mut detailed_arguments = default_arguments;
    detailed_arguments.extend(["--show-missing".to_string(), "--show-unmatched".to_string()]);
    command()
        .args(detailed_arguments)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""missing_roms": ["#))
        .stdout(predicate::str::contains(r#""galaga""#))
        .stdout(predicate::str::contains(r#""unmatched_roms": ["#))
        .stdout(predicate::str::contains(r#""unknown""#));
}

#[test]
fn rom_stats_accepts_comma_separated_categories() {
    let directory = fixture();
    let mut arguments = vec![
        "rom".to_string(),
        "stats".to_string(),
        directory.path().join("roms").display().to_string(),
        "--category".to_string(),
        "'Maze','Shooter'".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];
    arguments.extend(source_args(directory.path()));

    command()
        .args(arguments)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""total": 2"#))
        .stdout(predicate::str::contains(r#""Maze""#))
        .stdout(predicate::str::contains(r#""Shooter""#));
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
        "--destination".to_string(),
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
        .stdout(predicate::str::contains(r#""category": "Maze""#))
        .stdout(predicate::str::contains(r#""subcategory": "Chase""#))
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
        "--destination".to_string(),
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
        "--name".to_string(),
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
        "--category".to_string(),
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
        .args(["completions", "--shell", "powershell"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Register-ArgumentCompleter"));
}

#[test]
fn source_clear_previews_by_default() {
    command()
        .args(["source", "clear", "--target", "mame", "--output", "json"])
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

#[test]
fn reports_metadata_and_rom_folder_errors() {
    let directory = tempfile::tempdir().unwrap();
    let missing_roms = directory.path().join("missing-roms");
    let mut arguments = vec![
        "rom".to_string(),
        "list".to_string(),
        missing_roms.display().to_string(),
    ];
    arguments.extend(source_args(directory.path()));
    command()
        .args(arguments)
        .assert()
        .failure()
        .stderr(predicate::str::contains("I/O error"));

    command()
        .args([
            "catalog",
            "list",
            "--mame-xml",
            directory.path().join("missing.xml").to_str().unwrap(),
            "--catver",
            directory.path().join("catver.ini").to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("I/O error"));
}

fn flow_command(fixture: &FlowFixture, arguments: &[&str]) -> Command {
    let mut command = command_in(fixture.directory.path());
    command
        .args(arguments)
        .arg("--mame-xml")
        .arg(&fixture.mame_xml)
        .arg("--catver")
        .arg(&fixture.catver)
        .args(["--output", "json"]);
    command
}

fn output_json(output: &Output, code: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid JSON: {error}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn flow_json(fixture: &FlowFixture, arguments: &[&str]) -> Value {
    output_json(&flow_command(fixture, arguments).output().unwrap(), 0)
}

#[test]
fn populated_read_flows_match_shared_semantic_expectations() {
    let fixture = FlowFixture::new();
    let roms = fixture.roms.to_str().unwrap();
    let all = flow_json(&fixture, &["rom", "list", roms, "--status", "all"]);
    support::assert_initial_scan(&all, false);
    assert_eq!(
        names(&flow_json(&fixture, &["rom", "list", roms])),
        ["galaga", "pacman"]
    );
    assert_eq!(
        names(&flow_json(
            &fixture,
            &["rom", "list", roms, "--status", "missing"]
        )),
        ["missing"]
    );
    assert_eq!(
        names(&flow_json(
            &fixture,
            &["rom", "list", roms, "--status", "unmatched"]
        )),
        ["unknown"]
    );
    let filtered = flow_json(&fixture, &["rom", "list", roms, "--category", "Maze"]);
    assert_eq!(names(&filtered), ["pacman"]);
    assert_eq!(filtered[0]["region"], "usa");
    let catalog = flow_json(&fixture, &["catalog", "list"]);
    let mut expected = support::expected_collection();
    expected.as_array_mut().unwrap().pop();
    for row in expected.as_array_mut().unwrap() {
        row["available"] = serde_json::json!(false);
    }
    assert_eq!(support::normalize_roms(&catalog, false), expected);
    let metadata = flow_json(&fixture, &["catalog", "show", "--name", "PACMAN"]);
    assert_eq!(names(&metadata), ["pacman"]);
    assert_eq!(metadata[0]["available"], false);
    assert_eq!(metadata[0]["category"], "Maze");
    assert_eq!(metadata[0]["subcategory"], "Chase");
    assert_eq!(
        support::normalize_categories(&flow_json(&fixture, &["category", "list"]), false),
        support::expected_categories()
    );
    assert_eq!(
        names(&flow_json(
            &fixture,
            &["category", "show", "--category", "Maze"]
        )),
        ["pacman", "missing"]
    );
    let stats = flow_json(
        &fixture,
        &["rom", "stats", roms, "--show-missing", "--show-unmatched"],
    );
    assert_eq!(
        stats,
        serde_json::json!({
            "total": 4, "available": 2, "missing": 1, "unmatched": 1,
            "by_category": {
                "Maze": {"total": 2, "available": 1, "missing": 1, "unmatched": 0},
                "Shooter": {"total": 1, "available": 1, "missing": 0, "unmatched": 0},
                "Unknown": {"total": 1, "available": 0, "missing": 0, "unmatched": 1}
            },
            "missing_roms": ["missing"], "unmatched_roms": ["unknown"]
        })
    );
    fixture.assert_unrelated_files();
}

#[test]
fn rom_show_checks_availability_and_unknown_name_errors() {
    let fixture = FlowFixture::new();
    let roms = fixture.roms.to_str().unwrap();
    for (name, available, catalogued) in [
        ("PACMAN", true, true),
        ("missing", false, true),
        ("unknown", true, false),
    ] {
        let rows = flow_json(&fixture, &["rom", "show", roms, "--name", name]);
        assert_eq!(rows.as_array().unwrap().len(), 1);
        assert_eq!(rows[0]["name"], name.to_ascii_lowercase());
        assert_eq!(rows[0]["available"], available);
        assert_eq!(rows[0]["catalogued"], catalogued);
        assert_eq!(rows[0]["path"].is_string(), available);
    }
    flow_command(&fixture, &["rom", "show", roms, "--name", "absent"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("ROM 'absent' was not found"));
}

#[test]
fn audit_reports_exact_findings_and_respects_failure_thresholds() {
    let fixture = FlowFixture::new();
    let roms = fixture.roms.to_str().unwrap();
    fs::write(fixture.roms.join("pacman.7z"), b"duplicate bytes").unwrap();
    fs::write(
        &fixture.catver,
        "[Category]\npacman=Maze / Chase\nmissing=Maze / Chase\n",
    )
    .unwrap();
    for threshold in ["warning", "error"] {
        let output = flow_command(&fixture, &["rom", "audit", roms, "--fail-on", threshold])
            .output()
            .unwrap();
        let findings = output_json(&output, 2);
        assert_eq!(
            findings
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    (
                        row["kind"].as_str().unwrap(),
                        row["name"].as_str().unwrap(),
                        row["level"].as_str().unwrap(),
                    )
                })
                .collect::<Vec<_>>(),
            [
                ("duplicate", "pacman", "error"),
                ("metadata", "galaga", "warning"),
                ("missing", "missing", "warning"),
                ("unmatched", "unknown", "error")
            ]
        );
        assert!(
            findings[0]["message"]
                .as_str()
                .unwrap()
                .contains("multiple archives resolve")
        );
        assert_eq!(
            findings[1]["message"],
            "available ROM has no category metadata"
        );
        assert_eq!(
            findings[2]["message"],
            "catalog entry has no archive in the ROM directory"
        );
        assert_eq!(
            findings[3]["message"],
            "archive has no matching runnable catalog entry"
        );
    }
    fs::remove_file(fixture.roms.join("pacman.7z")).unwrap();
    fs::remove_file(fixture.roms.join("unknown.zip")).unwrap();
    fs::write(&fixture.catver, support::CATVER).unwrap();
    let warning = flow_command(&fixture, &["rom", "audit", roms, "--fail-on", "warning"])
        .output()
        .unwrap();
    let error = flow_command(&fixture, &["rom", "audit", roms, "--fail-on", "error"])
        .output()
        .unwrap();
    assert_eq!(output_json(&warning, 2), output_json(&error, 0));
    fs::write(fixture.roms.join("missing.zip"), b"previously missing").unwrap();
    assert_eq!(
        flow_json(&fixture, &["rom", "audit", roms]),
        serde_json::json!([])
    );
}

#[test]
fn move_and_delete_preview_execute_and_rescan_preserve_file_bytes() {
    for action in ["move", "delete"] {
        let fixture = FlowFixture::new();
        let destination = fixture.directory.path().join("moved");
        let mut args = vec![
            "rom",
            action,
            fixture.roms.to_str().unwrap(),
            "--name",
            "pacman",
        ];
        if action == "move" {
            args.extend(["--destination", destination.to_str().unwrap()]);
        }
        let preview = flow_json(&fixture, &args);
        assert_eq!(names(&preview), ["pacman"]);
        assert_eq!(preview[0]["action"], action);
        assert_eq!(preview[0]["state"], "preview");
        assert_eq!(
            fs::read(fixture.roms.join("pacman.zip")).unwrap(),
            PACMAN_BYTES
        );
        assert!(!destination.exists());
        fixture.assert_unrelated_files();
        support::assert_initial_scan(
            &flow_json(
                &fixture,
                &[
                    "rom",
                    "list",
                    fixture.roms.to_str().unwrap(),
                    "--status",
                    "all",
                ],
            ),
            false,
        );
        if action == "move" {
            fs::create_dir(&destination).unwrap();
            fs::write(destination.join("notes.txt"), support::NOTE_BYTES).unwrap();
        }
        args.push("--execute");
        let executed = flow_json(&fixture, &args);
        assert_eq!(names(&executed), ["pacman"]);
        assert_eq!(executed[0]["state"], "executed");
        assert!(!fixture.roms.join("pacman.zip").exists());
        if action == "move" {
            assert_eq!(
                fs::read(destination.join("pacman.zip")).unwrap(),
                PACMAN_BYTES
            );
            assert_eq!(
                fs::read(destination.join("notes.txt")).unwrap(),
                support::NOTE_BYTES
            );
            assert_eq!(fs::read_dir(&destination).unwrap().count(), 2);
        } else {
            assert!(!destination.exists());
        }
        fixture.assert_unrelated_files();
        support::assert_rescan(
            &flow_json(
                &fixture,
                &[
                    "rom",
                    "list",
                    fixture.roms.to_str().unwrap(),
                    "--status",
                    "all",
                ],
            ),
            false,
        );
    }
}

#[test]
fn move_collisions_leave_source_and_destination_unchanged() {
    let fixture = FlowFixture::new();
    let destination = fixture.directory.path().join("collision");
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("pacman.zip"), b"existing destination").unwrap();
    for execute in [false, true] {
        let mut args = vec![
            "rom",
            "move",
            fixture.roms.to_str().unwrap(),
            "--name",
            "pacman",
            "--destination",
            destination.to_str().unwrap(),
        ];
        if execute {
            args.push("--execute");
        }
        flow_command(&fixture, &args)
            .assert()
            .code(1)
            .stdout("")
            .stderr(predicate::str::contains("target already exists"));
        assert_eq!(
            fs::read(fixture.roms.join("pacman.zip")).unwrap(),
            PACMAN_BYTES
        );
        assert_eq!(
            fs::read(destination.join("pacman.zip")).unwrap(),
            b"existing destination"
        );
        fixture.assert_unrelated_files();
    }
}

#[test]
fn isolated_cache_clear_previews_and_executes_only_selected_files() {
    for target in ["mame", "catver", "all"] {
        let directory = tempfile::tempdir().unwrap();
        let cache = directory.path().join("cache");
        fs::create_dir(&cache).unwrap();
        fs::write(cache.join("mame.xml"), support::MAME_XML).unwrap();
        fs::write(cache.join("catver.ini"), support::CATVER).unwrap();
        fs::write(cache.join("notes.txt"), support::NOTE_BYTES).unwrap();
        let args = ["source", "clear", "--target", target, "--output", "json"];
        let preview = output_json(
            &command_in(directory.path()).args(args).output().unwrap(),
            0,
        );
        let expected_names: Vec<&str> = match target {
            "mame" => vec!["mame.xml"],
            "catver" => vec!["catver.ini"],
            _ => vec!["mame.xml", "catver.ini"],
        };
        assert_eq!(names(&preview), expected_names);
        for row in preview.as_array().unwrap() {
            assert_eq!(row["state"], "preview");
            assert_eq!(row["action"], "clear");
            assert_eq!(
                row["source"],
                cache.join(row["name"].as_str().unwrap()).to_str().unwrap()
            );
        }
        assert_eq!(
            fs::read(cache.join("mame.xml")).unwrap(),
            support::MAME_XML.as_bytes()
        );
        assert_eq!(
            fs::read(cache.join("catver.ini")).unwrap(),
            support::CATVER.as_bytes()
        );
        let executed = output_json(
            &command_in(directory.path())
                .args(args)
                .arg("--execute")
                .output()
                .unwrap(),
            0,
        );
        assert_eq!(names(&executed), expected_names);
        for row in executed.as_array().unwrap() {
            assert_eq!(row["state"], "executed");
        }
        for (name, bytes) in [
            ("mame.xml", support::MAME_XML),
            ("catver.ini", support::CATVER),
        ] {
            if expected_names.contains(&name) {
                assert!(!cache.join(name).exists());
            } else {
                assert_eq!(fs::read(cache.join(name)).unwrap(), bytes.as_bytes());
            }
        }
        assert_eq!(
            fs::read(cache.join("notes.txt")).unwrap(),
            support::NOTE_BYTES
        );
        let repeated = output_json(
            &command_in(directory.path())
                .args(args)
                .arg("--execute")
                .output()
                .unwrap(),
            0,
        );
        for row in repeated.as_array().unwrap() {
            assert_eq!(row["state"], "skipped");
        }
    }
}

#[test]
fn mame_extraction_refresh_and_cache_reuse_preserve_last_valid_metadata() {
    let fixture = FlowFixture::new();
    let executable = fixture
        .directory
        .path()
        .join(format!("fake-mame{}", std::env::consts::EXE_SUFFIX));
    let mut compiler = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    compiler
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/support/fake_mame.rs"))
        .arg("-o")
        .arg(&executable)
        .timeout(Duration::from_secs(60))
        .assert()
        .success();
    let calls = fixture.directory.path().join("calls.txt");
    let mut extract = command_in(fixture.directory.path());
    extract
        .args(["catalog", "list", "--output", "json"])
        .arg("--catver")
        .arg(&fixture.catver)
        .arg("--mame-executable")
        .arg(&executable)
        .env("FAKE_MAME_CALLS", &calls)
        .env("FAKE_MAME_XML", &fixture.mame_xml);
    let mut rows = output_json(&extract.output().unwrap(), 0);
    assert_eq!(names(&rows), ["galaga", "missing", "pacman"]);
    let cache = fixture.directory.path().join("cache");
    assert_eq!(
        fs::read(cache.join("mame.xml")).unwrap(),
        support::MAME_XML.as_bytes()
    );
    fs::write(cache.join("catver.ini"), support::CATVER).unwrap();
    let cached = output_json(
        &command_in(fixture.directory.path())
            .args(["catalog", "list", "--output", "json"])
            .output()
            .unwrap(),
        0,
    );
    assert_eq!(cached, rows);
    assert_eq!(fs::read_to_string(&calls).unwrap(), "-listxml\n");
    let refreshed_xml = support::MAME_XML.replace("Pac-Man (USA)", "Pac-Man Updated (USA)");
    fs::write(&fixture.mame_xml, &refreshed_xml).unwrap();
    rows[2]["description"] = serde_json::json!("Pac-Man Updated (USA)");
    for mode in ["success", "fail", "invalid", "empty", "encoding"] {
        let mut refresh = command_in(fixture.directory.path());
        refresh
            .args(["source", "refresh", "--target", "mame", "--output", "json"])
            .arg("--mame-executable")
            .arg(&executable)
            .env("FAKE_MAME_CALLS", &calls)
            .env("FAKE_MAME_XML", &fixture.mame_xml)
            .env("FAKE_MAME_MODE", mode);
        let output = refresh.output().unwrap();
        if mode == "success" {
            let details = output_json(&output, 0);
            assert_eq!(details.as_array().unwrap().len(), 2);
            for (index, source, filename, size) in [
                (0, "mame", "mame.xml", refreshed_xml.len()),
                (1, "catver", "catver.ini", support::CATVER.len()),
            ] {
                assert_eq!(details[index]["source"], source);
                assert_eq!(details[index]["state"], "available");
                assert_eq!(
                    details[index]["path"],
                    cache.join(filename).to_str().unwrap()
                );
                assert_eq!(details[index]["size_bytes"], size);
            }
        } else {
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            let diagnostic = String::from_utf8(output.stderr).unwrap();
            let expected = match mode {
                "fail" => "failed to run",
                "encoding" => "not valid UTF-8",
                _ => "failed to parse XML",
            };
            assert!(diagnostic.contains(expected), "{diagnostic}");
        }
        assert_eq!(
            fs::read(cache.join("mame.xml")).unwrap(),
            refreshed_xml.as_bytes()
        );
        assert_eq!(
            output_json(
                &command_in(fixture.directory.path())
                    .args(["catalog", "list", "--output", "json"])
                    .output()
                    .unwrap(),
                0
            ),
            rows
        );
        assert_eq!(
            fs::read(cache.join("catver.ini")).unwrap(),
            support::CATVER.as_bytes()
        );
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 2);
    }
    assert_eq!(fs::read_to_string(&calls).unwrap(), "-listxml\n".repeat(6));
    fixture.assert_unrelated_files();
}
