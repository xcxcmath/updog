# 🌭 Updog

Just for Rust practice.

Cross-platform CLI/TUI tool for managing system and package updates seamlessly.
*Update everything. Everywhere. Efficiently.*

## 🚀 Features

- Cross-platform support – Works on macOS, Linux, and Windows
- Configurable updates – Define custom update commands via YAML configuration
- Interactive & TUI support – Run in a terminal with interactive options
- Logging & Dry-run – Logs everything, with an option to preview changes
- Interactive prompts – Directly respond to package manager prompts during updates
- Flexible configuration – Supports optional check commands and simplified string syntax

## 📦 Installation

**Not implemented yet**

## ⚙️ Configuration

Updog uses a YAML configuration file to define update commands. By default, it looks for `updog.yaml` in the following locations:
- `$HOME/.config/updog/updog.yaml` (Linux/macOS)
- `%APPDATA%\updog\updog.yaml` (Windows)

### Configuration Format

```yaml
commands:
  # Modern format with subcommands support
  - id: homebrew
    subcommands:
      - id: default
        check: "brew outdated"
        update: "brew upgrade"
      - id: cask
        check: "brew outdated --cask"
        update: "brew upgrade --cask"

  # Simple format - directly define commands on the package manager
  - id: npm
    check: "npm outdated -g"
    update: "npm update -g"
    
  # Command sequence - run multiple commands in sequence
  - id: rust
    subcommands:
      - id: default
        update:
          - "rustup update"
          - "cargo install-update -a"

  # Update-only manager (no check command)
  - id: cargo
    update: "cargo install-update -a"
    
  # Check-only manager (no update command)
  - id: docker-images
    check: "docker images --format '{{.Repository}}:{{.Tag}}' | grep -v '<none>' | xargs -I{} docker pull {} 2>&1 | grep -v 'up to date'"
```

Three ways to configure package managers:

1. **Modern format with subcommands**: Define a package manager with multiple subcommands
   - `id`: Unique identifier for the package manager
   - `subcommands`: List of subcommands, each with its own check/update commands
     - `id`: Unique identifier for the subcommand (use "default" for the default subcommand)
     - `check`: Command to check for updates (optional)
     - `update`: Command to perform the actual update (optional)

2. **Simple format**: Define update/check commands directly on the package manager
   - `id`: Unique identifier for the package manager
   - `check`: Command to check for updates (optional)
   - `update`: Command to perform the actual update (optional)

3. **Command sequence**: For commands that need to run multiple steps in sequence
   ```yaml
   commands:
     - id: rust
       update:
         - "rustup update"
         - "cargo install-update -a"
   ```

## 🛠️ Usage

**Basic Commands**

```bash
# Check for updates across all configured commands
updog check

# Update everything
updog update

# Check specific package manager
updog check homebrew

# Update specific package manager
updog update homebrew

# Check specific package manager with subcommand
updog check homebrew:cask

# Update specific package manager with subcommand
updog update homebrew:cask

# Show what will be updated without executing
updog check --dry-run

# Use a custom configuration file
updog --config config.yaml update
```

**Interactive Mode**

When running `updog update`, the tool will pass through any interactive prompts from the package managers. This allows you to directly respond to confirmation prompts (like "Do you want to continue? [Y/n]") during the update process.

**TUI Mode**
```bash
updog tui
```
> **Note:** TUI mode is currently not implemented yet. This feature is planned for future releases.

## 🔧 Development

Requirements:
- Rust 1.70 or higher

Clone the repository:
```bash
git clone https://github.com/xcxcmath/updog.git
cd updog
```

Build the project:
```bash
cargo build
```

Install locally:
```bash
cargo install --path .
```

## 📝 License

This project is licensed under the MIT License - see the LICENSE file for details.
