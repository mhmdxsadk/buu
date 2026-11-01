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
    /// Neither says "buu": the terminal shows that itself, or buu adds it.
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

/// Desktop notifications, sent as escape sequences to the user's terminal.
///
/// Does nothing unless the terminal is one buu knows how to notify through
/// and stdout or stderr is attached to it.
pub struct Notifications {
    channel: Option<Channel>,
}

struct Channel {
    protocol: Protocol,
    stream: Stream,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Protocol {
    /// `OSC 777;notify;title;body`: WezTerm.
    Osc777,
    /// `OSC 777` as above. Ghostty always adds the tab title as a second line,
    /// regular weight when the body is empty, so buu sends "BUU" as the title,
    /// no body, and puts the text in the tab title. The password notification
    /// cannot (see `prompt`), so it is "BUU: Password required." with the
    /// tab title emptied by `Notifications::begin`.
    ///
    /// Ghostty applies a tab title change after a 75 ms debounce but reads the
    /// title as soon as it receives the notification, so buu waits between
    /// the two and must not put the old title back. The shell resets it at
    /// its next prompt. The password notification travels inside sudo's
    /// prompt as a single write, which is why the blank title is set early.
    Ghostty,
    /// `OSC 9;message`, which has no title: iTerm2.
    Osc9,
}

#[derive(Copy, Clone)]
enum Stream {
    Stderr,
    Stdout,
}

impl Protocol {
    fn detect(term_program: Option<&str>, term: Option<&str>) -> Option<Self> {
        match (term_program, term) {
            (Some("ghostty"), _) | (_, Some("xterm-ghostty")) => Some(Protocol::Ghostty),
            (Some("WezTerm"), _) => Some(Protocol::Osc777),
            (Some("iTerm.app"), _) => Some(Protocol::Osc9),
            _ => None,
        }
    }

    /// The notification as one write, which is how it travels inside sudo's
    /// password prompt. Ghostty shows the tab title under the headline.
    fn sequence(self, title: &str, body: &str) -> String {
        let (title, body) = (clean(title), clean(body));
        match self {
            Protocol::Osc777 | Protocol::Ghostty => {
                format!("\x1b]777;notify;{title};{body}\x07")
            }
            // iTerm2 shows no title of its own, so say who is speaking.
            Protocol::Osc9 => {
                let text: Vec<&str> = [title.as_str(), body.as_str()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect();
                format!("\x1b]9;buu: {}\x07", text.join(". "))
            }
        }
    }

    /// The notification in the order buu writes it, with a pause between the
    /// parts (see `write_notification`).
    fn sequences(self, title: &str, body: &str, plain: bool) -> Vec<String> {
        match self {
            Protocol::Ghostty => vec![
                tab_title(&ghostty_text(title, body, plain)),
                "\x1b]777;notify;BUU;\x07".to_string(),
            ],
            _ => vec![self.sequence(title, body)],
        }
    }

    /// The notification as sudo's password prompt carries it: one write, with
    /// no time to wait for a tab title change. In Ghostty "BUU" and the text
    /// share the bold title, and the empty tab title `begin` set leaves no
    /// line under it.
    fn prompt(self, title: &str, body: &str, plain: bool) -> String {
        match self {
            Protocol::Ghostty => format!(
                "\x1b]777;notify;BUU: {};\x07",
                clean(&ghostty_text(title, body, plain))
            ),
            _ => self.sequence(title, body),
        }
    }
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
        let protocol = Protocol::detect(
            env::var("TERM_PROGRAM").ok().as_deref(),
            env::var("TERM").ok().as_deref(),
        );
        let stream = if io::stderr().is_terminal() {
            Some(Stream::Stderr)
        } else if io::stdout().is_terminal() {
            Some(Stream::Stdout)
        } else {
            None
        };
        Self {
            channel: protocol
                .zip(stream)
                .map(|(protocol, stream)| Channel { protocol, stream }),
        }
    }

    /// Call once before any brew command runs. In Ghostty the banner's second
    /// line is the tab title, or the working directory when the title is
    /// empty. The password notification cannot wait for a title change, so
    /// buu forgets the directory and empties the title beforehand. The shell
    /// restores both at its next prompt.
    pub fn begin(&self) {
        if let Some(channel) = &self.channel
            && channel.protocol == Protocol::Ghostty
        {
            self.write(channel, &["\x1b]7;\x07\x1b]2;\x07".to_string()]);
        }
    }

    pub fn send(&self, message: Message) {
        let Some(channel) = &self.channel else {
            return;
        };
        let (title, body) = message.text();
        let sequences = channel
            .protocol
            .sequences(&title, &body, message.is_plain());
        self.write(channel, &sequences);
    }

    fn write(&self, channel: &Channel, sequences: &[String]) {
        let result = match channel.stream {
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
        if let Some(channel) = &self.channel
            && env::var_os("SUDO_ASKPASS").is_none()
        {
            let message = Message::PasswordRequired;
            let (title, body) = message.text();
            let mut prompt =
                OsString::from(channel.protocol.prompt(&title, &body, message.is_plain()));
            prompt.push(env::var_os("SUDO_PROMPT").unwrap_or_else(|| "Password: ".into()));
            command.env("SUDO_PROMPT", prompt);
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
    fn detects_supported_terminals() {
        for (term_program, term, expected) in [
            (
                Some("ghostty"),
                Some("xterm-ghostty"),
                Some(Protocol::Ghostty),
            ),
            (None, Some("xterm-ghostty"), Some(Protocol::Ghostty)),
            (
                Some("WezTerm"),
                Some("xterm-256color"),
                Some(Protocol::Osc777),
            ),
            (
                Some("iTerm.app"),
                Some("xterm-256color"),
                Some(Protocol::Osc9),
            ),
            (Some("Apple_Terminal"), Some("xterm-256color"), None),
            (Some("tmux"), Some("tmux-256color"), None),
            (None, None, None),
        ] {
            assert_eq!(Protocol::detect(term_program, term), expected);
        }
    }

    #[test]
    fn sequences_follow_each_terminals_protocol() {
        assert_eq!(
            Protocol::Osc777.sequence("Finished.", ""),
            "\x1b]777;notify;Finished.;\x07"
        );
        assert_eq!(
            Protocol::Osc9.sequence("Finished.", ""),
            "\x1b]9;buu: Finished.\x07"
        );
        assert_eq!(
            Protocol::Osc777.sequence("All done", "Finished."),
            "\x1b]777;notify;All done;Finished.\x07"
        );
        assert_eq!(
            Protocol::Osc9.sequence("All done", "Finished."),
            "\x1b]9;buu: All done. Finished.\x07"
        );
        assert_eq!(
            Protocol::Osc777.sequences("All done", "Finished.", true),
            ["\x1b]777;notify;All done;Finished.\x07"]
        );
    }

    #[test]
    fn ghostty_puts_the_text_in_the_tab_title() {
        assert_eq!(
            Protocol::Ghostty.sequences("Finished", "", false),
            ["\x1b]2;Finished\x07", "\x1b]777;notify;BUU;\x07"]
        );
        assert_eq!(
            Protocol::Ghostty.sequences("Upgrade failed", "Check the terminal.", true),
            [
                "\x1b]2;Upgrade failed. Check the terminal.\x07",
                "\x1b]777;notify;BUU;\x07"
            ]
        );
        assert_eq!(
            Protocol::Ghostty.prompt("Password required", "", true),
            "\x1b]777;notify;BUU: Password required.;\x07"
        );
    }

    #[test]
    fn ghostty_waits_for_the_tab_title_before_notifying() {
        let sequences = Protocol::Ghostty.sequences("Finished", "", false);
        assert_eq!(sequences.len(), 2);
        let mut written = Vec::new();
        let start = std::time::Instant::now();
        write_notification(&mut written, &sequences).unwrap();
        assert!(start.elapsed() >= PART_DELAY);
        assert_eq!(String::from_utf8(written).unwrap(), sequences.concat());
    }

    #[test]
    fn notification_text_cannot_escape_the_payload() {
        for protocol in [Protocol::Osc777, Protocol::Ghostty, Protocol::Osc9] {
            let sequence = protocol.sequence("Done\x07\x1b;\n", "Body\x07\x1b;\n");
            assert!(sequence.ends_with("Body\x07"));
            assert_eq!(sequence.matches('\x07').count(), 1);
            assert_eq!(sequence.matches('\x1b').count(), 1);
        }
        let osc777 = Protocol::Osc777.sequence("a;b", "c;d");
        assert_eq!(osc777.matches(';').count(), 3);
        assert_eq!(
            Protocol::Ghostty.sequences("Done\x07\x1b;\n", "Body\x07\x1b;\n", false),
            ["\x1b]2;Done\x07", "\x1b]777;notify;BUU;\x07"]
        );
        assert_eq!(
            Protocol::Ghostty.prompt("Done\x07\x1b;\n", "Body\x07\x1b;\n", true),
            "\x1b]777;notify;BUU: Done. Body.;\x07"
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
