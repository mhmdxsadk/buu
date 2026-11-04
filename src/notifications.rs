use std::env;
use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// What buu tells the user about a run.
pub enum Message {
    Done,
    Failed(&'static str),
    PasswordRequired,
}

impl Message {
    /// The notification's headline and its details, which may be empty.
    ///
    /// Neither says "buu": the notification title already does.
    fn text(&self) -> (String, String) {
        match self {
            Message::Done => ("Homebrew is up to date".to_string(), String::new()),
            Message::Failed(step) => (
                format!("{} failed", capitalize(step)),
                "Check the terminal for details.".to_string(),
            ),
            Message::PasswordRequired => ("Password required".to_string(), String::new()),
        }
    }

    /// Whether Ghostty shows the text as one regular line under "BUU". Done
    /// is instead the bold second line, which reads as a headline.
    fn is_plain(&self) -> bool {
        !matches!(self, Message::Done)
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The headline and details as sentences: "Upgrade failed. Check the terminal."
fn sentences(title: &str, body: &str) -> String {
    let mut text = [title, body]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(". ");
    if !text.ends_with('.') {
        text.push('.');
    }
    text
}

/// Desktop notifications, sent as escape sequences to Ghostty.
///
/// Does nothing unless the terminal is Ghostty, stdout or stderr is attached
/// to it, and `BUU_NO_NOTIFY` is unset or empty.
///
/// Each notification ends with a bell (`BELL`) so that Ghostty also marks the
/// tab when the notification comes from a tab you are not looking at. While
/// Ghostty is the frontmost app, that is all buu sends: no banner. The
/// password notification cannot make that choice, because sudo prints its
/// prompt later (see `prompt`), so it always includes the banner.
///
/// Ghostty always adds the tab title as a second line, regular weight when
/// the body is empty, so buu sends "BUU" as the title with no body and puts
/// the text in the tab title. The password notification cannot (see
/// `prompt`), so it is "BUU: Password required." with the tab title emptied
/// by `Notifications::begin`.
///
/// Ghostty applies a tab title change after a 75 ms debounce but reads the
/// title as soon as it receives the notification, so buu waits between the
/// two and must not put the old title back. The shell resets it at its next
/// prompt. The password notification travels inside sudo's prompt as a single
/// write, which is why the blank title is set early.
pub struct Notifications {
    stream: Option<Stream>,
}

#[derive(Copy, Clone)]
enum Stream {
    Stderr,
    Stdout,
}

fn is_ghostty(term_program: Option<&str>, term: Option<&str>) -> bool {
    term_program == Some("ghostty") || term == Some("xterm-ghostty")
}

const GHOSTTY_BUNDLE_ID: &str = "com.mitchellh.ghostty";

/// Whether Ghostty is the app in front. macOS sets `__CFBundleIdentifier` for
/// programs started from an app, which also covers other Ghostty builds.
/// Anything buu cannot find out counts as "not in front", so the banner is
/// sent.
fn ghostty_is_frontmost() -> bool {
    let own = env::var("__CFBundleIdentifier").unwrap_or_else(|_| GHOSTTY_BUNDLE_ID.to_string());
    frontmost_bundle_id().is_some_and(|front| front == own)
}

fn frontmost_bundle_id() -> Option<String> {
    let front = lsappinfo(&["front"])?;
    let info = lsappinfo(&["info", "-only", "bundleid", front.trim()])?;
    parse_bundle_id(&info)
}

fn lsappinfo(args: &[&str]) -> Option<String> {
    let output = Command::new("lsappinfo")
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Reads the `bundleID="..."` line of `lsappinfo info` output.
fn parse_bundle_id(info: &str) -> Option<String> {
    info.lines().find_map(|line| {
        let id = line.trim().strip_prefix("bundleID=\"")?;
        Some(id.strip_suffix('"')?.to_string())
    })
}

/// Ghostty's in-app signal for a tab you are not looking at: a bell emoji on
/// the tab title and one Dock bounce, both cleared when you return to the tab.
/// Ghostty enables both by default (`bell-features`).
const BELL: &str = "\x07";

/// The notification in the order buu writes it, with a pause between the
/// parts (see `write_notification`).
///
/// With `banner` false, Ghostty is the frontmost app, so buu sends only the
/// tab title and the bell.
fn sequences(title: &str, body: &str, plain: bool, banner: bool) -> Vec<String> {
    let tab_title = tab_title(&ghostty_text(title, body, plain));
    if banner {
        vec![tab_title, format!("\x1b]777;notify;BUU;\x07{BELL}")]
    } else {
        vec![format!("{tab_title}{BELL}")]
    }
}

/// The notification as sudo's password prompt carries it: one write, with no
/// time to wait for a tab title change. "BUU" and the text share the bold
/// title, and the empty tab title `begin` set leaves no line under it.
fn prompt(title: &str, body: &str, plain: bool) -> String {
    format!(
        "\x1b]777;notify;BUU: {};\x07{BELL}",
        clean(&ghostty_text(title, body, plain))
    )
}

/// The single line Ghostty shows under "BUU".
fn ghostty_text(title: &str, body: &str, plain: bool) -> String {
    if plain {
        sentences(title, body)
    } else {
        title.to_string()
    }
}

fn tab_title(title: &str) -> String {
    format!("\x1b]2;{}\x07", clean(title))
}

/// Keeps notification text inside the payload, even if future callers supply
/// dynamic text containing terminal controls or field delimiters.
fn clean(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_control() && *ch != ';')
        .collect()
}

impl Notifications {
    pub fn new() -> Self {
        let ghostty = is_ghostty(
            env::var("TERM_PROGRAM").ok().as_deref(),
            env::var("TERM").ok().as_deref(),
        );
        let disabled = env::var_os("BUU_NO_NOTIFY").is_some_and(|value| !value.is_empty());
        let stream = if !ghostty || disabled {
            None
        } else if io::stderr().is_terminal() {
            Some(Stream::Stderr)
        } else if io::stdout().is_terminal() {
            Some(Stream::Stdout)
        } else {
            None
        };
        Self { stream }
    }

    /// Call once before any brew command runs. The banner's second line is
    /// the tab title, or the working directory when the title is empty. The
    /// password notification cannot wait for a title change, so buu forgets
    /// the directory and empties the title beforehand. The shell restores
    /// both at its next prompt.
    pub fn begin(&self) {
        if let Some(stream) = self.stream {
            self.write(stream, &["\x1b]7;\x07\x1b]2;\x07".to_string()]);
        }
    }

    pub fn send(&self, message: Message) {
        let Some(stream) = self.stream else {
            return;
        };
        let (title, body) = message.text();
        let banner = !ghostty_is_frontmost();
        self.write(
            stream,
            &sequences(&title, &body, message.is_plain(), banner),
        );
    }

    fn write(&self, stream: Stream, sequences: &[String]) {
        let result = match stream {
            Stream::Stderr => write_notification(io::stderr().lock(), sequences),
            Stream::Stdout => write_notification(io::stdout().lock(), sequences),
        };
        if let Err(error) = result {
            eprintln!("Warning: could not send desktop notification: {error}");
        }
    }

    pub fn brew_command(&self) -> Command {
        let mut command = Command::new("brew");
        command.stdin(Stdio::inherit());
        // sudo writes its prompt directly to /dev/tty, even when brew's output
        // is captured. Cached credentials never display this prompt. Leave
        // askpass helpers untouched: their prompts are not terminal output.
        if self.stream.is_some() && env::var_os("SUDO_ASKPASS").is_none() {
            let message = Message::PasswordRequired;
            let (title, body) = message.text();
            let mut text = OsString::from(prompt(&title, &body, message.is_plain()));
            text.push(env::var_os("SUDO_PROMPT").unwrap_or_else(|| "Password: ".into()));
            command.env("SUDO_PROMPT", text);
        }
        command
    }
}

/// Longer than Ghostty's 75 ms tab title debounce.
const PART_DELAY: Duration = Duration::from_millis(150);

fn write_notification(mut writer: impl Write, sequences: &[String]) -> io::Result<()> {
    for (index, sequence) in sequences.iter().enumerate() {
        if index > 0 {
            thread::sleep(PART_DELAY);
        }
        writer.write_all(sequence.as_bytes())?;
        writer.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ghostty_only() {
        for (term_program, term, expected) in [
            (Some("ghostty"), Some("xterm-ghostty"), true),
            (None, Some("xterm-ghostty"), true),
            (Some("WezTerm"), Some("xterm-256color"), false),
            (Some("iTerm.app"), Some("xterm-256color"), false),
            (Some("Apple_Terminal"), Some("xterm-256color"), false),
            (Some("tmux"), Some("tmux-256color"), false),
            (None, None, false),
        ] {
            assert_eq!(is_ghostty(term_program, term), expected);
        }
    }

    #[test]
    fn the_text_goes_in_the_tab_title() {
        assert_eq!(
            sequences("Finished", "", false, true),
            ["\x1b]2;Finished\x07", "\x1b]777;notify;BUU;\x07\x07"]
        );
        assert_eq!(
            sequences("Upgrade failed", "Check the terminal.", true, true),
            [
                "\x1b]2;Upgrade failed. Check the terminal.\x07",
                "\x1b]777;notify;BUU;\x07\x07"
            ]
        );
        assert_eq!(
            prompt("Password required", "", true),
            "\x1b]777;notify;BUU: Password required.;\x07\x07"
        );
    }

    #[test]
    fn the_frontmost_ghostty_gets_only_the_tab_title_and_the_bell() {
        assert_eq!(
            sequences("Finished", "", false, false),
            ["\x1b]2;Finished\x07\x07"]
        );
    }

    #[test]
    fn reads_the_bundle_id_from_lsappinfo() {
        let info = " [ NULL ]  ASN:0x0-0x156cb6b6: (in front) \n    bundleID=\"com.mitchellh.ghostty\"\n    bundle path=[ NULL ] \n";
        assert_eq!(
            parse_bundle_id(info).as_deref(),
            Some("com.mitchellh.ghostty")
        );
        assert_eq!(parse_bundle_id("bundleID=[ NULL ]"), None);
        assert_eq!(parse_bundle_id(""), None);
    }

    #[test]
    fn ghostty_waits_for_the_tab_title_before_notifying() {
        let sequences = sequences("Finished", "", false, true);
        assert_eq!(sequences.len(), 2);
        let mut written = Vec::new();
        let start = std::time::Instant::now();
        write_notification(&mut written, &sequences).unwrap();
        assert!(start.elapsed() >= PART_DELAY);
        assert_eq!(String::from_utf8(written).unwrap(), sequences.concat());
    }

    #[test]
    fn notification_text_cannot_escape_the_payload() {
        assert_eq!(
            sequences("Done\x07\x1b;\n", "Body\x07\x1b;\n", false, true),
            ["\x1b]2;Done\x07", "\x1b]777;notify;BUU;\x07\x07"]
        );
        assert_eq!(
            prompt("Done\x07\x1b;\n", "Body\x07\x1b;\n", true),
            "\x1b]777;notify;BUU: Done. Body.;\x07\x07"
        );
    }

    #[test]
    fn messages_do_not_repeat_the_app_name() {
        for (message, title, body) in [
            (Message::Done, "Homebrew is up to date", ""),
            (
                Message::Failed("upgrade"),
                "Upgrade failed",
                "Check the terminal for details.",
            ),
            (Message::PasswordRequired, "Password required", ""),
        ] {
            let (actual, body_actual) = message.text();
            assert_eq!(actual, title);
            assert_eq!(body_actual, body);
            assert!(!actual.to_lowercase().contains("buu"));
            assert!(!body_actual.to_lowercase().contains("buu"));
        }
    }
}
