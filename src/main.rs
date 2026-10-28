use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use anyhow::Context;
use clap::Parser;
use owo_colors::OwoColorize;

mod notifications;
use notifications::{Message, Notifications};

type Result<T> = anyhow::Result<T>;

#[derive(Copy, Clone)]
enum BrewStep {
    Update,
    Upgrade,
    Cleanup,
}

impl BrewStep {
    fn label(self) -> &'static str {
        match self {
            BrewStep::Update => "Updating Homebrew...",
            BrewStep::Upgrade => "Upgrading Homebrew...",
            BrewStep::Cleanup => "Cleaning up...",
        }
    }

    fn name(self) -> &'static str {
        match self {
            BrewStep::Update => "update",
            BrewStep::Upgrade => "upgrade",
            BrewStep::Cleanup => "cleanup",
        }
    }

    fn run(self, notifications: &Notifications) -> Result<()> {
        match self {
            BrewStep::Update => run_update(notifications),
            BrewStep::Upgrade => run_upgrade(notifications),
            BrewStep::Cleanup => run_cleanup(notifications),
        }
    }

    /// Lines brew prints that add nothing to buu's output.
    fn is_noise(self, line: &str) -> bool {
        match self {
            BrewStep::Update => {
                line.starts_with("==> Updating Homebrew") || line.trim() == "Already up-to-date."
            }
            BrewStep::Upgrade | BrewStep::Cleanup => false,
        }
    }
}

#[derive(Parser)]
#[command(
    name = "buu",
    version,
    about = "Update, upgrade, and clean Homebrew in one command."
)]
struct Cli {}

fn main() -> Result<()> {
    let _cli = Cli::parse();

    let notifications = Notifications::new();
    let result = run(&notifications);
    if result.is_ok() {
        notifications.send(Message::Done);
    }

    result
}

fn run(notifications: &Notifications) -> Result<()> {
    let steps = [BrewStep::Update, BrewStep::Upgrade, BrewStep::Cleanup];

    let total = steps.len();

    for (idx, step) in steps.iter().copied().enumerate() {
        print_step_header(idx + 1, total, step.label());
        if let Err(error) = step.run(notifications) {
            notifications.send(Message::Failed(step.name()));
            return Err(error.context(format!("Homebrew {} failed", step.name())));
        }
        println!();
    }

    Ok(())
}

fn print_step_header(index: usize, total: usize, label: &str) {
    println!(
        "{} {}",
        format!("[{}/{}]", index, total).bright_cyan().bold(),
        label.bold(),
    );
}

fn print_no_changes() {
    println!("    {}", "No changes.".yellow().italic());
}

fn print_stderr_block(block: &str) {
    if !block.trim().is_empty() {
        let styled = block.red();
        eprintln!("{styled}");
    }
}

fn run_update(notifications: &Notifications) -> Result<()> {
    stream(BrewStep::Update, notifications.brew_command().arg("update"))
}

fn run_upgrade(notifications: &Notifications) -> Result<()> {
    if !detect_outdated(notifications)? {
        print_no_changes();
        return Ok(());
    }

    stream(
        BrewStep::Upgrade,
        notifications.brew_command().arg("upgrade"),
    )
}

fn detect_outdated(notifications: &Notifications) -> Result<bool> {
    let output = notifications
        .brew_command()
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .args(["outdated", "--quiet"])
        .output()?;

    if !output.status.success() {
        print_stderr_block(&String::from_utf8_lossy(&output.stderr));
        anyhow::bail!("brew outdated failed with status {}", output.status);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(!stdout.trim().is_empty())
}

fn run_cleanup(notifications: &Notifications) -> Result<()> {
    stream(
        BrewStep::Cleanup,
        notifications.brew_command().args(["cleanup", "-s"]),
    )
}

/// Runs a brew command, printing its tidied output as it arrives.
///
/// Errors (stderr) are printed in red. A failing command has already shown
/// its output by the time this returns the error.
fn stream(step: BrewStep, command: &mut Command) -> Result<()> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let (sender, receiver) = mpsc::channel();
    let readers = [
        spawn_reader(
            child.stdout.take().context("brew stdout is not piped")?,
            false,
            sender.clone(),
        ),
        spawn_reader(
            child.stderr.take().context("brew stderr is not piped")?,
            true,
            sender,
        ),
    ];

    let mut tidy = Tidy::new(step);
    for (is_stderr, line) in receiver {
        for out in tidy.feed(&line) {
            if is_stderr {
                eprintln!("{}", out.red());
            } else {
                println!("{out}");
            }
        }
    }

    for reader in readers {
        let _ = reader.join();
    }

    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("brew {} failed with status {status}", step.name());
    }

    if !tidy.printed {
        print_no_changes();
    }

    Ok(())
}

/// Sends each line of `stream` to `sender`, tagged with whether it came from stderr.
fn spawn_reader(
    stream: impl Read + Send + 'static,
    is_stderr: bool,
    sender: mpsc::Sender<(bool, String)>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut buf = Vec::new();
        while matches!(reader.read_until(b'\n', &mut buf), Ok(n) if n > 0) {
            let line = String::from_utf8_lossy(&buf).trim_end().to_string();
            buf.clear();
            if sender.send((is_stderr, line)).is_err() {
                break;
            }
        }
    })
}

/// Decides which of brew's lines buu prints, one line at a time.
struct Tidy {
    step: BrewStep,
    printed: bool,
    pending_blanks: usize,
}

impl Tidy {
    fn new(step: BrewStep) -> Self {
        Self {
            step,
            printed: false,
            pending_blanks: 0,
        }
    }

    /// Returns the lines to print for `line`, in order.
    ///
    /// Noise is dropped. Blank lines are dropped for `brew update` and, for
    /// the other steps, kept only between printed lines.
    fn feed(&mut self, line: &str) -> Vec<String> {
        let line = line.trim_end();
        if self.step.is_noise(line) {
            return Vec::new();
        }

        if line.trim().is_empty() {
            if self.printed && !matches!(self.step, BrewStep::Update) {
                self.pending_blanks += 1;
            }
            return Vec::new();
        }

        let mut out = vec![String::new(); self.pending_blanks];
        self.pending_blanks = 0;
        out.push(line.to_string());
        self.printed = true;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed_all(step: BrewStep, lines: &[&str]) -> Vec<String> {
        let mut tidy = Tidy::new(step);
        lines.iter().flat_map(|line| tidy.feed(line)).collect()
    }

    #[test]
    fn update_drops_header_blanks_and_up_to_date() {
        let mut tidy = Tidy::new(BrewStep::Update);
        for line in ["==> Updating Homebrew...", "", "Already up-to-date."] {
            assert!(tidy.feed(line).is_empty());
        }
        assert!(!tidy.printed);
    }

    #[test]
    fn update_keeps_changes_without_blank_lines() {
        let lines = [
            "==> Updating Homebrew...",
            "Updated 1 tap.",
            "",
            "==> New Formulae",
        ];
        assert_eq!(
            feed_all(BrewStep::Update, &lines),
            ["Updated 1 tap.", "==> New Formulae"]
        );
    }

    #[test]
    fn upgrade_keeps_inner_blank_lines_but_trims_the_edges() {
        let lines = ["", "==> Upgrading 1 outdated package:", "", "", "done", ""];
        assert_eq!(
            feed_all(BrewStep::Upgrade, &lines),
            ["==> Upgrading 1 outdated package:", "", "", "done"]
        );
    }

    #[test]
    fn trailing_whitespace_is_trimmed() {
        assert_eq!(
            feed_all(BrewStep::Cleanup, &["Removing: x   "]),
            ["Removing: x"]
        );
    }
}
