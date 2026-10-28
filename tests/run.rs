#![cfg(unix)]

mod common;

use std::io::Read;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::Duration;

use common::{Fixture, text};

#[test]
fn runs_every_step_in_order() {
    let fixture = Fixture::new();
    let output = fixture.command().output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        fixture.read("steps"),
        "update\noutdated\nupgrade\ncleanup\n"
    );
    let text = text(&output);
    assert_eq!(text.matches("No changes.").count(), 3);
    for header in [
        "Updating Homebrew...",
        "Upgrading Homebrew...",
        "Cleaning up...",
    ] {
        assert!(text.contains(header), "{header}");
    }
}

#[test]
fn a_failing_step_stops_the_run_and_shows_brew_output() {
    for (failing, step, expected_steps) in [
        ("update", "update", "update\n"),
        ("outdated", "upgrade", "update\noutdated\n"),
        ("upgrade", "upgrade", "update\noutdated\nupgrade\n"),
        ("cleanup", "cleanup", "update\noutdated\nupgrade\ncleanup\n"),
    ] {
        let fixture = Fixture::new();
        let output = fixture
            .command()
            .env("FAIL_STEP", failing)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{failing}");
        assert_eq!(fixture.read("steps"), expected_steps, "{failing}");
        let text = text(&output);
        assert!(text.contains("brew test failure"), "{failing}");
        assert!(
            text.contains(&format!("Homebrew {step} failed")),
            "{failing}"
        );
    }
}

#[test]
fn output_appears_while_brew_is_still_running() {
    let fixture = Fixture::new();
    let mut child = fixture
        .command()
        .env("STREAM", "1")
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
            if bytes.ends_with(b"first line\n") {
                sender.send(()).unwrap();
            }
        }
        String::from_utf8_lossy(&bytes).into_owned()
    });
    let received = receiver.recv_timeout(Duration::from_secs(5));
    let still_running = child.try_wait().unwrap().is_none();
    fixture.release();
    let status = child.wait().unwrap();
    let output = reader.join().unwrap();
    assert!(received.is_ok(), "first line never arrived: {output}");
    assert!(
        still_running,
        "brew finished before the first line was read"
    );
    assert!(status.success());
    assert!(output.find("first line").unwrap() < output.find("last line").unwrap());
}
