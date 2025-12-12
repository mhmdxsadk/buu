# BUU

**buu** is a small, fast Rust-powered CLI tool that updates, upgrades, and cleans your Homebrew installation in one clean and satisfying command. It runs `brew update`, `brew upgrade`, and `brew cleanup -s` with pretty presentation layer on top of it.

---

## Features

- Single command to maintain Homebrew
- Colored, human-friendly output

---

## Installation

Install via Homebrew:

```
brew tap mhmdxsadk/tools
brew install buu
```

---

## Usage

Run:

```
buu
```

What it does:

1. **Update** Homebrew
2. **Upgrade** outdated packages (formulae + casks)
3. **Clean up** old artifacts

---

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

---

## Why “buu”?

I built this as an excuse to learn Rust.  
I know I could have pulled this off with a simple bash script — but where’s the fun in that?

---

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
