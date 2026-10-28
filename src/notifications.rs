use std::env;
use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::process::{Command, Stdio};

/// What buu tells the user about a run.
pub enum Message {
    Done,
    Failed(&'static str),
    PasswordRequired,
}

impl Message {
    /// The notification's headline and its message.
    ///
    /// Neither says "buu": terminals such as Ghostty already show the tab
    /// title, which is "buu" while buu runs.
    fn text(&self) -> (String, String) {
        match self {
            Message::Done => (
                "All done".to_string(),
                "Homebrew is updated, upgraded and cleaned up.".to_string(),
            ),
            Message::Failed(step) => (
                format!("Homebrew {step} failed"),
                "Check the terminal for details.".to_string(),
            ),
            Message::PasswordRequired => (
                "Password required".to_string(),
                "Homebrew is waiting for your password.".to_string(),
            ),
        }
    }
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
    /// `OSC 777;notify;title;body`: Ghostty, WezTerm.
    Osc777,
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
            (Some("ghostty" | "WezTerm"), _) | (_, Some("xterm-ghostty")) => Some(Protocol::Osc777),
            (Some("iTerm.app"), _) => Some(Protocol::Osc9),
            _ => None,
        }
    }

    fn sequence(self, title: &str, body: &str) -> String {
        // Keep notification text inside the payload, even if future callers
        // supply dynamic text containing terminal controls or field delimiters.
        let clean = |text: &str| -> String {
            text.chars()
                .filter(|ch| !ch.is_control() && *ch != ';')
                .collect()
        };
        let (title, body) = (clean(title), clean(body));
        match self {
            Protocol::Osc777 => format!("\x1b]777;notify;{title};{body}\x07"),
            // iTerm2 shows no title of its own, so say who is speaking.
            Protocol::Osc9 => format!("\x1b]9;buu: {title}. {body}\x07"),
        }
    }
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

    pub fn send(&self, message: Message) {
        let Some(channel) = &self.channel else {
            return;
        };
        let (title, body) = message.text();
        let sequence = channel.protocol.sequence(&title, &body);
        let result = match channel.stream {
            Stream::Stderr => write_notification(io::stderr().lock(), &sequence),
            Stream::Stdout => write_notification(io::stdout().lock(), &sequence),
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
            let (title, body) = Message::PasswordRequired.text();
            let mut prompt = OsString::from(channel.protocol.sequence(&title, &body));
            prompt.push(env::var_os("SUDO_PROMPT").unwrap_or_else(|| "Password: ".into()));
            command.env("SUDO_PROMPT", prompt);
        }
        command
    }
}

fn write_notification(mut writer: impl Write, sequence: &str) -> io::Result<()> {
    writer.write_all(sequence.as_bytes())?;
    writer.flush()
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
                Some(Protocol::Osc777),
            ),
            (None, Some("xterm-ghostty"), Some(Protocol::Osc777)),
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
            Protocol::Osc777.sequence("All done", "Finished."),
            "\x1b]777;notify;All done;Finished.\x07"
        );
        assert_eq!(
            Protocol::Osc9.sequence("All done", "Finished."),
            "\x1b]9;buu: All done. Finished.\x07"
        );
    }

    #[test]
    fn notification_text_cannot_escape_the_payload() {
        for protocol in [Protocol::Osc777, Protocol::Osc9] {
            let sequence = protocol.sequence("Done\x07\x1b;\n", "Body\x07\x1b;\n");
            assert!(sequence.ends_with("Body\x07"));
            assert_eq!(sequence.matches('\x07').count(), 1);
            assert_eq!(sequence.matches('\x1b').count(), 1);
        }
        let osc777 = Protocol::Osc777.sequence("a;b", "c;d");
        assert_eq!(osc777.matches(';').count(), 3);
    }

    #[test]
    fn messages_have_a_headline_and_a_body_and_do_not_repeat_the_app_name() {
        for (message, title) in [
            (Message::Done, "All done"),
            (Message::Failed("upgrade"), "Homebrew upgrade failed"),
            (Message::PasswordRequired, "Password required"),
        ] {
            let (actual, body) = message.text();
            assert_eq!(actual, title);
            assert!(!body.is_empty());
            assert!(!actual.to_lowercase().contains("buu"));
            assert!(!body.to_lowercase().contains("buu"));
        }
    }
}
