# BUU

**buu** is a small, fast Rust-powered CLI tool that updates, upgrades, and cleans your Homebrew installation in one clean and satisfying command. It runs `brew update`, `brew upgrade`, and `brew cleanup -s` wrapped with a pretty presentation layer

## Features

- Single command to maintain Homebrew
- Colored, human-friendly output that appears as Homebrew works
- Desktop notifications when a run finishes, fails, or needs your password

## Installation

buu supports macOS. Install via Homebrew:

```
brew tap mhmdxsadk/tools
brew install buu
```

## Usage

Run:

```
buu
```

What it does:

1. **Update** Homebrew
2. **Upgrade** outdated packages (formulae + casks that Homebrew considers outdated)
3. **Clean up** old artifacts

The steps always run in this order. To run just one of them, use the matching
`brew` command. If a step fails, buu shows Homebrew's output, stops, and exits
with status 1.

### Notifications

In a supported terminal, buu sends a desktop notification:

- **Done** ("Homebrew is up to date"): every step completed.
- **Failed** (for example "Homebrew upgrade failed"): a step failed, and the
  notification names it. Details stay in the terminal.
- **Password required**: sudo is waiting for your password.

Supported terminals are [Ghostty](https://ghostty.org), WezTerm and iTerm2.
Ghostty is the one buu is tested in; WezTerm and iTerm2 use their own
notification escape sequences. iTerm2 has no notification titles, so it shows
`buu:` in front of the text, while Ghostty shows the tab title (`buu` while buu
runs) under the headline. Done has no headline, so `buu` is its top line.
Notifications are skipped in other terminals (including inside tmux or over
SSH) and when both stdout and stderr are redirected. A failure to send one never
changes the exit status.

The password notification is attached to sudo's own prompt, so it appears only
when sudo really asks, never for cached credentials. buu does not read or store
your password, and it keeps an existing `SUDO_PROMPT`. It is not sent when
`SUDO_ASKPASS` points at a graphical helper, or when
`HOMEBREW_SUDO_THROUGH_SUDO_USER` is set, because Homebrew then replaces sudo's
prompt.

There is no flag to turn notifications off. To silence them, use your
terminal's or macOS's settings. In Ghostty, set `desktop-notifications = false`;
on macOS, allow or block the terminal under System Settings → Notifications.
Terminals may hide banners while their window is focused, and macOS Focus modes
can hide them too.

## Example Output

```
[1/3] Updating Homebrew...
    No changes.

[2/3] Upgrading Homebrew...
==> Upgrading 1 outdated package:
opencode 1.0.147 -> 1.0.150
...

[3/3] Cleaning up...
    No changes.
```

## Why “buu”?

I built this as an excuse to learn Rust.  
I know I could have pulled this off with a simple bash script — but where’s the fun in that?

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
