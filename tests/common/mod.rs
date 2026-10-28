//! A fake `brew` on PATH, so buu can be run end to end without touching Homebrew.
//!
//! The fake records each subcommand it is asked to run, and is steered by
//! environment variables:
//!
//! - `FAIL_STEP`: subcommand that prints an error and exits with status 7.
//! - `STREAM`: `upgrade` prints a line, waits for `release`, then prints a last line.
//! - `ASK_PASSWORD`: `upgrade` writes sudo's prompt to the controlling terminal.
//! - `WAIT_FOR_RELEASE`: with `ASK_PASSWORD`, wait for `release` after prompting.
#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

pub struct Fixture(pub PathBuf);

impl Fixture {
    pub fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "buu-test-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("brew");
        fs::write(
            &path,
            r#"#!/bin/sh
printf '%s\n' "$1" >> "$TEST_DIR/steps"
printf '%s' "${SUDO_PROMPT-unset}" > "$TEST_DIR/prompt-$1"
wait_for_release() {
    count=0
    while [ ! -f "$TEST_DIR/release" ]; do
        /bin/sleep 0.01
        count=$((count + 1))
        if [ "$count" -ge 500 ]; then exit 9; fi
    done
}
if [ "$1" = "$FAIL_STEP" ]; then
    echo 'brew test failure' >&2
    exit 7
fi
if [ "$1" = outdated ]; then echo example-package; fi
if [ "$1" = upgrade ] && [ "$STREAM" = 1 ]; then
    echo 'first line'
    wait_for_release
    echo 'last line'
fi
if [ "$1" = upgrade ] && [ "$ASK_PASSWORD" = 1 ]; then
    # Model sudo: write the prompt directly to the controlling terminal.
    printf '%s' "${SUDO_PROMPT-Password: }" > /dev/tty
    if [ "$WAIT_FOR_RELEASE" = 1 ]; then wait_for_release; fi
fi
exit 0
"#,
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        Self(dir)
    }

    /// Points `command` at the fake brew with every behavior switched off.
    pub fn configure(&self, command: &mut Command) {
        command
            .env("PATH", &self.0)
            .env("TEST_DIR", &self.0)
            .env("FAIL_STEP", "")
            .env("STREAM", "0")
            .env("ASK_PASSWORD", "0")
            .env("WAIT_FOR_RELEASE", "0")
            .env_remove("SUDO_PROMPT")
            .env_remove("SUDO_ASKPASS")
            .stdin(Stdio::null());
    }

    /// buu itself, run against the fake brew.
    pub fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_buu"));
        self.configure(&mut command);
        command
    }

    pub fn read(&self, name: &str) -> String {
        fs::read_to_string(self.0.join(name)).unwrap()
    }

    /// Lets a fake brew that is waiting for `release` carry on.
    pub fn release(&self) {
        fs::write(self.0.join("release"), "").unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}
