#![cfg(target_os = "macos")]

mod common;

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use common::{Fixture, text};

const DONE: &str = "\x1b]777;notify;;Homebrew is up to date\x07";
const PASSWORD: &str =
    "\x1b]777;notify;Password required;Homebrew is waiting for your password.\x07";

fn failed(step: &str) -> String {
    format!("\x1b]777;notify;Homebrew {step} failed;Check the terminal for details.\x07")
}

/// buu run against the fake brew, as Ghostty. With `terminal`, buu's output
/// goes to a pseudo-terminal, as it would in a real session.
fn command(fixture: &Fixture, terminal: bool) -> Command {
    let binary = env!("CARGO_BIN_EXE_buu");
    let mut command = if terminal {
        let mut command = Command::new("/usr/bin/script");
        command.args(["-q", "/dev/null", binary]);
        command
    } else {
        Command::new(binary)
    };
    fixture.configure(&mut command);
    command
        .env("TERM_PROGRAM", "ghostty")
        .env("TERM", "xterm-ghostty");
    command
}

#[test]
fn success_sends_one_done_notification() {
    let fixture = Fixture::new();
    let output = command(&fixture, true).output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        fixture.read("steps"),
        "update\noutdated\nupgrade\ncleanup\n"
    );
    let text = text(&output);
    assert_eq!(text.matches(DONE).count(), 1);
    assert_eq!(text.matches("\x1b]777;").count(), 1);
    assert!(!text.contains(PASSWORD));
}

#[test]
fn each_failure_sends_one_notification_naming_the_step() {
    for (failing, step) in [
        ("update", "update"),
        ("outdated", "upgrade"),
        ("upgrade", "upgrade"),
        ("cleanup", "cleanup"),
    ] {
        let fixture = Fixture::new();
        let output = command(&fixture, true)
            .env("FAIL_STEP", failing)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{failing}");
        let text = text(&output);
        assert_eq!(text.matches(&failed(step)).count(), 1, "{failing}");
        assert_eq!(text.matches("\x1b]777;").count(), 1, "{failing}");
        assert!(!text.contains(DONE), "{failing}");
        assert!(text.contains("brew test failure"), "{failing}");
    }
}

#[test]
fn each_supported_terminal_gets_its_own_sequence() {
    for (term_program, term, done) in [
        ("WezTerm", "xterm-256color", DONE),
        (
            "iTerm.app",
            "xterm-256color",
            "\x1b]9;buu: Homebrew is up to date\x07",
        ),
    ] {
        let fixture = Fixture::new();
        let output = command(&fixture, true)
            .env("TERM_PROGRAM", term_program)
            .env("TERM", term)
            .output()
            .unwrap();
        assert!(output.status.success(), "{term_program}");
        assert_eq!(text(&output).matches(done).count(), 1, "{term_program}");
    }
}

#[test]
fn redirected_output_and_other_terminals_skip_notifications() {
    for terminal in [false, true] {
        let fixture = Fixture::new();
        let mut command = command(&fixture, terminal);
        if terminal {
            command
                .env("TERM_PROGRAM", "Apple_Terminal")
                .env("TERM", "xterm-256color");
        }
        let output = command.output().unwrap();
        assert!(output.status.success());
        let text = text(&output);
        assert!(!text.contains("\x1b]777;"));
        assert!(!text.contains("\x1b]9;"));
        assert_eq!(fixture.read("prompt-update"), "unset");
    }
}

#[test]
fn custom_password_prompt_is_preserved() {
    let fixture = Fixture::new();
    let output = command(&fixture, true)
        .env("SUDO_PROMPT", "Password for %u: ")
        .env("ASK_PASSWORD", "1")
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = text(&output);
    assert!(text.contains(&format!("{PASSWORD}Password for %u: ")));
    assert_eq!(text.matches(PASSWORD).count(), 1);
}

#[test]
fn askpass_helper_prompt_is_not_modified() {
    let fixture = Fixture::new();
    let output = command(&fixture, true)
        .env("SUDO_ASKPASS", "/custom/helper")
        .env("SUDO_PROMPT", "Custom prompt: ")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(fixture.read("prompt-upgrade"), "Custom prompt: ");
    assert!(text(&output).contains(DONE));
}

#[test]
fn password_notification_arrives_while_brew_is_waiting() {
    let fixture = Fixture::new();
    let mut child = command(&fixture, true)
        .env("ASK_PASSWORD", "1")
        .env("WAIT_FOR_RELEASE", "1")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut byte = [0];
        while stdout.read(&mut byte).unwrap() != 0 {
            bytes.push(byte[0]);
            if bytes.ends_with(PASSWORD.as_bytes()) {
                sender.send(()).unwrap();
            }
        }
        String::from_utf8_lossy(&bytes).into_owned()
    });
    let received = receiver.recv_timeout(Duration::from_secs(5));
    let waiting = child.try_wait().unwrap().is_none();
    fixture.release();
    let status = child.wait().unwrap();
    let output = reader.join().unwrap();
    assert!(received.is_ok(), "password notification missing: {output}");
    assert!(waiting, "brew finished before input was supplied");
    assert!(status.success());
    assert!(output.find(PASSWORD).unwrap() < output.find(DONE).unwrap());
}
