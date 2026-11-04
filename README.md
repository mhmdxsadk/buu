# BUU

**buu** is a small, fast Rust-powered CLI tool that updates, upgrades, and cleans your Homebrew installation in one clean and satisfying command. It runs `brew update`, `brew upgrade`, and `brew cleanup -s` wrapped with a pretty presentation layer

## Features

- Single command to maintain Homebrew
- Colored, human-friendly output that appears as Homebrew works
- Desktop notifications in Ghostty when a run finishes, fails, or needs your password

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

## Notifications

buu sends a desktop notification when:

- a run finishes
- a step fails
- sudo asks for your password

Notifications work only in [Ghostty](https://ghostty.org).
Each one also rings the terminal bell. If you are in another tab, Ghostty
puts a 🔔 on the buu tab and bounces the Dock icon once. Both clear when you
return to the tab. Ghostty plays no sound by default. To hear one, add this
to your Ghostty config, using any sound file you like:

```
bell-features = no-system,audio
bell-audio-path = /System/Library/Sounds/Ping.aiff
```

Without `audio`, `bell-features = system` plays the macOS alert sound instead.

While Ghostty is the frontmost app, buu sends only the bell and no banner.
If another app is in front, you get the banner too. The password notification
always includes the banner, because buu cannot tell when sudo will ask.

To turn all of it off, set `BUU_NO_NOTIFY=1`, in your `~/.zshrc` config file:

```
export BUU_NO_NOTIFY=1
```

## Why “buu”?

I built this as an excuse to use Rust. A bash script would have done the same job.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
