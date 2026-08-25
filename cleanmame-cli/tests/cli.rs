use assert_cmd::{Command, cargo::cargo_bin};

#[test]
fn help_lists_commands() {
    let mut cmd = Command::new(cargo_bin!("cleanmame-cli"));

    let output = cmd
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8(output).unwrap();
    for command in ["scan", "query", "filter", "move", "delete", "report"] {
        assert!(help.contains(command), "missing command {command}");
    }
}
