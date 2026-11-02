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

### Notifications

buu sends a desktop notification when:

- a run finishes
- a step fails
- sudo asks for your password

Notifications work only in [Ghostty](https://ghostty.org). To turn them off, set
`desktop-notifications = false` in Ghostty.

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
