use std::process::Command;
use owo_colors::OwoColorize;

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

    fn run(self, ctx: &BrewContext) -> Result<()> {
        match self {
            BrewStep::Update => run_update(),
            BrewStep::Upgrade => run_upgrade(ctx),
            BrewStep::Cleanup => run_cleanup(),
        }
    }
}

struct BrewContext {
    is_outdated: bool,
}

impl BrewContext {
    fn detect() -> Result<Self> {
        let output = Command::new("brew")
            .args(["outdated", "--quiet"])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let is_outdated = !stdout.trim().is_empty();

        Ok(Self { is_outdated })
    }
}

fn main() -> Result<()> {
    let ctx = BrewContext::detect()?;

    let steps = [
        BrewStep::Update,
        BrewStep::Upgrade,
        BrewStep::Cleanup,
    ];

    let total = steps.len();

    for (idx, step) in steps.iter().copied().enumerate() {
        print_step_header(idx + 1, total, step.label());
        step.run(&ctx)?;
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

fn print_stdout_block(block: &str) {
    if !block.trim().is_empty() {
        println!("{block}");
    }
}

fn print_stderr_block(block: &str) {
    if !block.trim().is_empty() {
        let styled = block.red();
        eprintln!("{styled}");
    }
}

fn run_update() -> Result<()> {
    let output = Command::new("brew")
        .arg("update")
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let mut lines: Vec<String> = stdout
        .lines()
        .chain(stderr.lines())
        .map(|l| l.trim_end().to_string())
        .collect();

    lines.retain(|line| !line.starts_with("==> Updating Homebrew"));

    lines.retain(|line| !line.trim().is_empty());

    let only_already_up_to_date =
        !lines.is_empty() && lines.iter().all(|l| l.trim() == "Already up-to-date.");

    if lines.is_empty() || only_already_up_to_date {
        print_no_changes();
        return Ok(());
    }

    let joined = lines.join("\n");
    print_stdout_block(&joined);

    Ok(())
}

fn run_upgrade(ctx: &BrewContext) -> Result<()> {
    if !ctx.is_outdated {
        print_no_changes();
        return Ok(());
    }

    let output = Command::new("brew")
        .arg("upgrade")
        .output()?;

    if !output.status.success() {
        anyhow::bail!("brew upgrade failed with status {}", output.status);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let stdout_trimmed = stdout.trim();
    let stderr_trimmed = stderr.trim();

    if stdout_trimmed.is_empty() && stderr_trimmed.is_empty() {
        print_no_changes();
        return Ok(());
    }

    print_stdout_block(stdout_trimmed);
    print_stderr_block(stderr_trimmed);

    Ok(())
}

fn run_cleanup() -> Result<()> {
    let output = Command::new("brew")
        .args(["cleanup", "-s"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let stdout_trimmed = stdout.trim();
    let stderr_trimmed = stderr.trim();

    if stdout_trimmed.is_empty() && stderr_trimmed.is_empty() {
        print_no_changes();
        return Ok(());
    }

    print_stdout_block(stdout_trimmed);
    print_stderr_block(stderr_trimmed);

    Ok(())
}
