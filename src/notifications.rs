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
    fn text(&self) -> String {
        match self {
            Message::Done => "Done".to_string(),
            Message::Failed(step) => format!("Failed: {step}"),
            Message::PasswordRequired => "Password required".to_string(),
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
    /// `OSC 9;body`, which has no title: iTerm2.
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

    fn sequence(self, message: &str) -> String {
        // Keep notification text inside the payload, even if future callers
        // supply dynamic text containing terminal controls or field delimiters.
        let message: String = message
            .chars()
            .filter(|ch| !ch.is_control() && *ch != ';')
            .collect();
        match self {
            Protocol::Osc777 => format!("\x1b]777;notify;buu;{message}\x07"),
            Protocol::Osc9 => format!("\x1b]9;buu: {message}\x07"),
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
        let sequence = channel.protocol.sequence(&message.text());
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
            let mut prompt =
                OsString::from(channel.protocol.sequence(&Message::PasswordRequired.text()));
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
            Protocol::Osc777.sequence("Done"),
            "\x1b]777;notify;buu;Done\x07"
        );
        assert_eq!(Protocol::Osc9.sequence("Done"), "\x1b]9;buu: Done\x07");
    }

    #[test]
    fn notification_text_cannot_escape_the_payload() {
        for protocol in [Protocol::Osc777, Protocol::Osc9] {
            let sequence = protocol.sequence("Done\x07\x1b;\n");
            assert!(sequence.ends_with("Done\x07"));
            assert_eq!(sequence.matches('\x07').count(), 1);
            assert_eq!(sequence.matches('\x1b').count(), 1);
        }
    }

    #[test]
    fn message_text_names_the_failed_step() {
        assert_eq!(Message::Done.text(), "Done");
        assert_eq!(Message::Failed("upgrade").text(), "Failed: upgrade");
        assert_eq!(Message::PasswordRequired.text(), "Password required");
    }
}
