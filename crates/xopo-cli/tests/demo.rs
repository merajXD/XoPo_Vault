//! Runs the real `xopo` binary, the way a user would: the demo command must
//! produce a vault that a second process can unlock, with no prompts and no real
//! seed phrase anywhere.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const PASSWORD: &str = "demo-password-123";
const PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const BITCOIN_ADDRESS: &str = "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu";

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let mut path = std::env::temp_dir();
        path.push(format!("xopo-cli-{name}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path).expect("temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn xopo(arguments: &[&str], password: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_xopo"));
    command.args(arguments);
    command.env_remove("XOPO_PASSWORD");
    command.env_remove("XOPO_NEW_PASSWORD");
    if let Some(password) = password {
        command.env("XOPO_PASSWORD", password);
    }
    command.output().expect("run xopo")
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "command failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn the_demo_command_builds_a_vault_a_second_process_can_open() {
    let dir = TempDir::new("demo");
    let target = dir.path().join("nested").join("demo");
    let target_text = target.to_str().expect("utf-8 path");

    let created = stdout(&xopo(&["demo", target_text, "--password", PASSWORD], None));
    assert!(created.contains("Demo vault created."), "{created}");

    let database = target.join("demo.xopo");
    assert!(database.exists(), "database was created");
    assert!(
        target.join("demo.xopo.key").exists(),
        "key file was created"
    );

    // A separate invocation, the way the user would reopen it.
    let listed = stdout(&xopo(
        &["unlock", database.to_str().expect("utf-8 path")],
        Some(PASSWORD),
    ));
    assert!(listed.contains("wallets  : 1"), "{listed}");
    assert!(listed.contains(BITCOIN_ADDRESS), "{listed}");

    let wallet_id = created
        .lines()
        .find_map(|line| line.trim().strip_prefix("wallet   : "))
        .and_then(|rest| rest.split_whitespace().next())
        .expect("wallet id in the demo output")
        .to_owned();

    let shown = stdout(&xopo(
        &[
            "wallet",
            "show",
            database.to_str().expect("utf-8 path"),
            "--id",
            &wallet_id,
            "--phrase",
        ],
        Some(PASSWORD),
    ));
    assert!(shown.contains(PHRASE), "{shown}");

    // The point of the whole exercise: the phrase is sealed, not stored as text.
    let bytes = fs::read(&database).expect("read database");
    let needle = b"abandon";
    assert!(
        !bytes.windows(needle.len()).any(|window| window == needle),
        "the recovery phrase must never be plaintext in the database"
    );
}

#[test]
fn a_wrong_password_fails_with_exit_code_two() {
    let dir = TempDir::new("wrong");
    let target = dir.path().join("demo");
    stdout(&xopo(
        &[
            "demo",
            target.to_str().expect("utf-8 path"),
            "--password",
            PASSWORD,
        ],
        None,
    ));

    let database = target.join("demo.xopo");
    let output = xopo(
        &["unlock", database.to_str().expect("utf-8 path")],
        Some("not-the-password"),
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("wrong password"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let printed = String::from_utf8_lossy(&output.stdout).to_lowercase();
    assert!(
        !printed.contains("wallet"),
        "a failed unlock must not print vault contents: {printed}"
    );
}

#[test]
fn the_demo_refuses_to_overwrite_an_existing_vault() {
    let dir = TempDir::new("exists");
    let target = dir.path().join("demo");
    let target_text = target.to_str().expect("utf-8 path");
    stdout(&xopo(&["demo", target_text, "--password", PASSWORD], None));

    let again = xopo(&["demo", target_text, "--password", PASSWORD], None);
    assert!(!again.status.success());
    assert!(
        String::from_utf8_lossy(&again.stderr).contains("already exists"),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
}
