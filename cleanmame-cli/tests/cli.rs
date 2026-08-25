use assert_cmd::Command;

#[test]
fn help_lists_commands() {
    let mut cmd = Command::cargo_bin("cleanmame-cli").unwrap();

    cmd.arg("--help").assert().success();
}
