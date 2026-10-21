//! Behavior of the `fsroneclick` command-line binary.

use std::process::{Command, Stdio};

#[test]
fn closed_stdout_does_not_crash_the_cli() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fsroneclick"))
        .arg("--help")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Close our end of stdout right away, like `fsroneclick --list-games | head` once head
    // has read enough. The child starts writing only after this.
    drop(child.stdout.take());

    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("panicked"), "stderr: {stderr}");
    assert!(output.status.success(), "exit status: {:?}", output.status);
}
