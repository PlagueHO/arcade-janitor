use std::{
    fs::{self, File},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use reqwest::blocking::Client;
use serde_json::{Value, json};

#[allow(dead_code)]
#[path = "../../../tests/support/mod.rs"]
mod support;

const PACKAGE_DIR: &str = "ARCADEJANITOR_RELEASE_PACKAGE_DIR";
const COMMAND_WAIT: Duration = Duration::from_secs(15);
const STARTUP_WAIT: Duration = Duration::from_secs(10);
const REQUEST_WAIT: Duration = Duration::from_secs(5);
const CLEANUP_WAIT: Duration = Duration::from_secs(5);

struct Package {
    cli: PathBuf,
    server: PathBuf,
    mame_xml: PathBuf,
    catver: PathBuf,
}

impl Package {
    fn load(directory: &Path) -> Result<Self> {
        let directory = directory
            .canonicalize()
            .with_context(|| format!("cannot open extracted package {}", directory.display()))?;
        let executable = |name| -> Result<PathBuf> {
            let path = directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
            ensure!(
                path.is_file(),
                "missing extracted executable {}",
                path.display()
            );
            Ok(path)
        };
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        Ok(Self {
            cli: executable("arcadejanitor")?,
            server: executable("arcadejanitor-mcp")?,
            mame_xml: fixtures.join("mame-100.xml").canonicalize()?,
            catver: fixtures.join("catver-100.ini").canonicalize()?,
        })
    }

    fn command(&self, executable: &Path, root: &Path) -> Command {
        let mut command = Command::new(executable);
        for name in support::inherited_application_settings() {
            command.env_remove(name);
        }
        command
            .envs(support::environment(root))
            .current_dir(root)
            .stdin(Stdio::null());
        command
    }

    fn sources(&self, command: &mut Command) {
        command
            .arg("--mame-xml")
            .arg(&self.mame_xml)
            .arg("--catver")
            .arg(&self.catver);
    }
}

struct Process {
    child: Child,
    label: String,
    stdout: PathBuf,
    stderr: PathBuf,
    stopped: bool,
}

impl Process {
    fn start(mut command: Command, root: &Path, label: &str) -> Result<Self> {
        let stdout = root.join(format!("{label}-stdout.txt"));
        let stderr = root.join(format!("{label}-stderr.txt"));
        command
            .stdout(File::create(&stdout)?)
            .stderr(File::create(&stderr)?);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let label = format!("{label}: {command:?}");
        let child = command.spawn().with_context(|| format!("start {label}"))?;
        Ok(Self {
            child,
            label,
            stdout,
            stderr,
            stopped: false,
        })
    }

    fn diagnostics(&self) -> String {
        let read = |path: &Path| match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => format!("cannot read {}: {error}", path.display()),
        };
        format!(
            "{} (PID {})\nstdout:\n{}\nstderr:\n{}",
            self.label,
            self.child.id(),
            read(&self.stdout),
            read(&self.stderr)
        )
    }

    fn output(mut self) -> Result<String> {
        let status = wait(&mut self.child, COMMAND_WAIT).with_context(|| self.diagnostics())?;
        self.stopped = true;
        ensure!(status.success(), "exit {status}\n{}", self.diagnostics());
        fs::read_to_string(&self.stdout).context("read command output")
    }

    fn stop(&mut self) -> Result<()> {
        if self.stopped {
            return Ok(());
        }
        if self.child.try_wait()?.is_none() {
            let mut command = tree_termination(self.child.id());
            let log = self.stderr.with_extension("cleanup.txt");
            command
                .stdin(Stdio::null())
                .stdout(File::create(&log)?)
                .stderr(File::options().append(true).open(&log)?);
            let mut termination = command.spawn().context("start process-tree cleanup")?;
            let status = match wait(&mut termination, CLEANUP_WAIT) {
                Ok(status) => status,
                Err(error) => {
                    termination
                        .kill()
                        .context("stop timed-out cleanup command")?;
                    wait(&mut termination, CLEANUP_WAIT)?;
                    return Err(error.context("process-tree cleanup timed out"));
                }
            };
            if !status.success() {
                let diagnostics = fs::read_to_string(log)?;
                ensure!(
                    self.child.try_wait()?.is_some(),
                    "process-tree cleanup exited {status}: {diagnostics}"
                );
                eprintln!(
                    "process-tree cleanup returned {status} after launcher exit:\n{diagnostics}"
                );
            }
        }
        wait(&mut self.child, CLEANUP_WAIT).context("reap package command")?;
        self.stopped = true;
        Ok(())
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if !self.stopped {
            eprintln!("{}", self.diagnostics());
            if let Err(error) = self.stop() {
                eprintln!("package process cleanup failed: {error:#}");
            }
        }
    }
}

#[cfg(windows)]
fn tree_termination(pid: u32) -> Command {
    let mut command = Command::new("taskkill.exe");
    command.args(["/PID", &pid.to_string(), "/T", "/F"]);
    command
}

#[cfg(unix)]
fn tree_termination(pid: u32) -> Command {
    let mut command = Command::new("/bin/kill");
    command.args(["-KILL", &format!("-{pid}")]);
    command
}

fn wait(child: &mut Child, timeout: Duration) -> Result<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        ensure!(
            Instant::now() < deadline,
            "PID {} exceeded {timeout:?}",
            child.id()
        );
        thread::sleep(Duration::from_millis(20));
    }
}

fn readiness(process: &mut Process, client: &Client, url: &str) -> Result<()> {
    let deadline = Instant::now() + STARTUP_WAIT;
    let mut last_response = String::from("no response");
    while Instant::now() < deadline {
        if let Some(status) = process.child.try_wait()? {
            bail!("launcher exited {status}\n{}", process.diagnostics());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        match client
            .get(format!("{url}/health"))
            .timeout(remaining.min(Duration::from_millis(200)))
            .send()
        {
            Ok(response) => {
                let status = response.status();
                match response.text() {
                    Ok(body) if status.is_success() && body == "ok (arcadejanitor)" => {
                        return Ok(());
                    }
                    Ok(body) => last_response = format!("HTTP {status}, body {body:?}"),
                    Err(error) => {
                        last_response = format!("HTTP {status}, cannot read health body: {error}");
                    }
                }
            }
            Err(error) => last_response = error.to_string(),
        }
        thread::sleep(Duration::from_millis(20));
    }
    bail!(
        "readiness timed out at {url}: {last_response}\n{}",
        process.diagnostics()
    )
}

fn assert_closed(address: SocketAddr) -> Result<()> {
    let deadline = Instant::now() + CLEANUP_WAIT;
    while TcpStream::connect_timeout(&address, Duration::from_millis(100)).is_ok() {
        ensure!(
            Instant::now() < deadline,
            "server still accepts connections at {address}"
        );
        thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

fn expected_roms() -> Value {
    json!([
        {"name": "game001", "description": "Game 001", "year": "1980",
         "manufacturer": "Fixture Co", "category": "Action", "subcategory": "Platform",
         "available": true, "catalogued": true},
        {"name": "game050", "description": "Game 050", "year": "1989",
         "manufacturer": "Fixture Co", "category": "Shooter", "subcategory": "Flying",
         "available": true, "catalogued": true},
        {"name": "game100", "description": "Game 100", "year": "1999",
         "manufacturer": "Fixture Co", "category": "Action", "subcategory": "Platform",
         "available": true, "catalogued": true}
    ])
}

fn expected_scan() -> Value {
    let rows = (1..=100)
        .map(|index| {
            let (category, subcategory) = match index % 3 {
                1 => ("Action", "Platform"),
                2 => ("Shooter", "Flying"),
                _ => ("Puzzle", "Tile"),
            };
            json!({
                "name": format!("game{index:03}"),
                "description": format!("Game {index:03}"),
                "year": (1980 + (index - 1) % 40).to_string(),
                "manufacturer": "Fixture Co", "category": category, "subcategory": subcategory,
                "available": matches!(index, 1 | 50 | 100), "catalogued": true
            })
        })
        .chain(std::iter::once(json!({
            "name": "not-in-catalog", "description": null, "year": null,
            "manufacturer": null, "category": null, "subcategory": null,
            "available": true, "catalogued": false
        })))
        .collect::<Vec<_>>();
    json!(rows)
}

fn check_server(
    package: &Package,
    root: &Path,
    roms: &Path,
    client: &Client,
    induce_failure: bool,
) -> Result<()> {
    let address = TcpListener::bind("127.0.0.1:0")?.local_addr()?;
    let url = format!("http://{address}");
    let result = (|| -> Result<()> {
        let mut command = package.command(&package.cli, root);
        command
            .args(["mcp", "start"])
            .arg(roms)
            .env("ARCADEJANITOR_MCP_ADDR", address.to_string());
        package.sources(&mut command);
        let mut server = Process::start(command, root, "mcp-launcher")?;
        readiness(&mut server, client, &url)?;
        let check = (|| -> Result<()> {
            ensure!(!induce_failure, "induced failure after server readiness");
            let response: Value = client
                .post(format!("{url}/mcp"))
                .json(&json!({
                    "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                    "params": {"name": "scan_roms", "arguments": {}}
                }))
                .send()?
                .error_for_status()?
                .json()?;
            ensure!(
                response["jsonrpc"] == "2.0" && response["id"] == 1,
                "{response}"
            );
            ensure!(response.get("error").is_none(), "{response}");
            ensure!(response["result"]["isError"] != true, "{response}");
            let text = response["result"]["content"][0]["text"]
                .as_str()
                .context("MCP result must contain JSON text")?;
            let payload: Value = serde_json::from_str(text)?;
            ensure!(payload["ok"] == true, "{payload}");
            ensure!(
                payload["roms"].is_array(),
                "missing MCP ROM records: {payload}"
            );
            ensure!(
                support::normalize_roms(&payload["roms"], true) == expected_scan(),
                "unexpected MCP scan: {payload}"
            );
            Ok(())
        })()
        .with_context(|| server.diagnostics());
        server.stop().with_context(|| server.diagnostics())?;
        check
    })();
    assert_closed(address)?;
    result
}

#[test]
#[ignore = "requires an extracted release archive; see docs/development.md"]
fn extracted_release_package() -> Result<()> {
    let directory = std::env::var_os(PACKAGE_DIR)
        .with_context(|| format!("{PACKAGE_DIR} must name an extracted release archive"))?;
    let package = Package::load(Path::new(&directory))?;
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let roms = root.join("roms");
    fs::create_dir(&roms)?;
    for name in [
        "game001.zip",
        "game050.7z",
        "game100.zip",
        "not-in-catalog.zip",
    ] {
        fs::write(roms.join(name), b"package fixture ROM")?;
    }

    for (executable, label, expected) in [
        (
            &package.cli,
            "cli-help",
            "Inspect and manage MAME ROM collections",
        ),
        (&package.server, "mcp-help", "Serve ArcadeJanitor MCP tools"),
    ] {
        let mut command = package.command(executable, root);
        command.arg("--help");
        let output = Process::start(command, root, label)?.output()?;
        ensure!(output.contains(expected), "unexpected {label}: {output}");
    }
    let mut query = package.command(&package.cli, root);
    query
        .args(["rom", "list"])
        .arg(&roms)
        .args(["--output", "json"]);
    package.sources(&mut query);
    let output = Process::start(query, root, "cli-query")?.output()?;
    let rows: Value = serde_json::from_str(&output)
        .with_context(|| format!("invalid CLI JSON from {}: {output}", package.cli.display()))?;
    ensure!(rows.is_array(), "CLI query must return an array: {rows}");
    ensure!(
        support::normalize_roms(&rows, false) == expected_roms(),
        "unexpected CLI query: {rows}"
    );

    let client = Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(1))
        .timeout(REQUEST_WAIT)
        .build()?;
    check_server(&package, root, &roms, &client, false)?;
    let failure = check_server(&package, root, &roms, &client, true)
        .expect_err("induced failure must be returned after cleanup");
    ensure!(
        failure.root_cause().to_string() == "induced failure after server readiness",
        "unexpected failure-path result: {failure:#}"
    );
    directory
        .close()
        .context("remove package test files after process cleanup")
}

#[test]
fn rejects_missing_package_executables() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let error = Package::load(directory.path())
        .err()
        .context("empty package was accepted")?;
    ensure!(
        error.to_string().contains("missing extracted executable"),
        "{error:#}"
    );
    fs::write(
        directory
            .path()
            .join(format!("arcadejanitor{}", std::env::consts::EXE_SUFFIX)),
        b"not an executable",
    )?;
    let error = Package::load(directory.path())
        .err()
        .context("package without an MCP server was accepted")?;
    ensure!(error.to_string().contains("arcadejanitor-mcp"), "{error:#}");
    Ok(())
}

fn start_process_fixture(root: &Path) -> Result<(Process, SocketAddr)> {
    let address = TcpListener::bind("127.0.0.1:0")?.local_addr()?;
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args([
            "--ignored",
            "--exact",
            "process_tree_fixture",
            "--nocapture",
        ])
        .env("ARCADEJANITOR_PACKAGE_HELPER_ROOT", root)
        .env("ARCADEJANITOR_PACKAGE_HELPER_ADDR", address.to_string())
        .env_remove("ARCADEJANITOR_PACKAGE_HELPER_CHILD");
    let mut process = Process::start(command, root, "process-tree-fixture")?;
    let deadline = Instant::now() + STARTUP_WAIT;
    while !root.join("ready").is_file() {
        ensure!(
            process.child.try_wait()?.is_none() && Instant::now() < deadline,
            "process-tree fixture did not start\n{}",
            process.diagnostics()
        );
        thread::sleep(Duration::from_millis(20));
    }
    Ok((process, address))
}

#[test]
fn readiness_retries_incomplete_health_responses() -> Result<()> {
    use std::io::{BufRead, BufReader, Write};

    let directory = tempfile::tempdir()?;
    let (mut process, process_address) = start_process_fixture(directory.path())?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let url = format!("http://{}", listener.local_addr()?);
    listener.set_nonblocking(true)?;
    let responder = thread::spawn(move || -> Result<()> {
        let deadline = Instant::now() + REQUEST_WAIT;
        for complete in [false, true] {
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        ensure!(Instant::now() < deadline, "health request did not arrive");
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => return Err(error.into()),
                }
            };
            stream.set_read_timeout(Some(REQUEST_WAIT))?;
            stream.set_write_timeout(Some(REQUEST_WAIT))?;
            let mut request = BufReader::new(&mut stream);
            loop {
                let mut line = String::new();
                ensure!(
                    request.read_line(&mut line)? != 0,
                    "incomplete HTTP request"
                );
                if line == "\r\n" {
                    break;
                }
            }
            let body = if complete { "ok (arcadejanitor)" } else { "ok" };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                "ok (arcadejanitor)".len()
            )?;
        }
        Ok(())
    });
    let client = Client::builder().no_proxy().timeout(REQUEST_WAIT).build()?;
    let result = readiness(&mut process, &client, &url);
    let response_result = responder.join().expect("health responder panicked");
    process.stop()?;
    assert_closed(process_address)?;
    result?;
    response_result?;
    directory.close().context("remove readiness fixture files")
}

#[test]
fn timeout_and_unwinding_stop_the_process_tree() -> Result<()> {
    for unwind in [false, true] {
        let directory = tempfile::tempdir()?;
        let (mut process, address) = start_process_fixture(directory.path())?;
        let error = wait(&mut process.child, Duration::from_millis(50))
            .expect_err("fixture must remain running until cleanup");
        ensure!(error.to_string().contains("exceeded"), "{error:#}");
        if unwind {
            let result = std::panic::catch_unwind(move || {
                let _guard = process;
                panic!("induced package process failure");
            });
            ensure!(result.is_err(), "expected induced panic");
        } else {
            process.stop()?;
        }
        assert_closed(address)?;
        directory
            .close()
            .context("remove process-tree fixture files")?;
    }
    Ok(())
}

#[test]
#[ignore = "subprocess fixture for timeout_and_unwinding_stop_the_process_tree"]
fn process_tree_fixture() -> Result<()> {
    let root = std::env::var_os("ARCADEJANITOR_PACKAGE_HELPER_ROOT")
        .context("process-tree fixture requires a test-owned root")?;
    if std::env::var_os("ARCADEJANITOR_PACKAGE_HELPER_CHILD").is_none() {
        let status = Command::new(std::env::current_exe()?)
            .args([
                "--ignored",
                "--exact",
                "process_tree_fixture",
                "--nocapture",
            ])
            .env("ARCADEJANITOR_PACKAGE_HELPER_CHILD", "1")
            .status()?;
        ensure!(status.success(), "fixture child exited {status}");
        bail!("fixture child exited before process-tree cleanup");
    }
    let address = std::env::var("ARCADEJANITOR_PACKAGE_HELPER_ADDR")?;
    let _listener = TcpListener::bind(address)?;
    fs::write(Path::new(&root).join("ready"), b"ready")?;
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
