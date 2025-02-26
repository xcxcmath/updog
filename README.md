# 🌭 Updog

Cross-platform CLI/TUI tool for managing system and package updates seamlessly.
*Update everything. Everywhere. Efficiently.*

## 🚀 Features

- Cross-platform support – Works on macOS, Linux, and Windows
- Configurable updates – Define custom update commands via YAML configuration
- Interactive & TUI support – Run in a terminal with interactive options
- Parallel execution – Runs updates efficiently using parallel processing
- Logging & Dry-run – Logs everything, with an option to preview changes
- Interactive prompts – Directly respond to package manager prompts during updates
- Flexible configuration – Supports optional check commands and simplified string syntax

## 📦 Installation

Linux/macOS:
```bash
curl -fsSL https://raw.githubusercontent.com/xcxcmath/updog/main/install.sh | bash
```

Windows:
```powershell
irm https://raw.githubusercontent.com/xcxcmath/updog/main/install.ps1 | iex
```

## ⚙️ Configuration

Updog uses a YAML configuration file to define update commands. By default, it looks for `updog.yaml` in the following locations:
- `$HOME/.config/updog/updog.yaml` (Linux/macOS)
- `%APPDATA%\updog\updog.yaml` (Windows)

### Configuration Format

```yaml
commands:
  # Standard format - specify both check and update commands
  homebrew:
    # Command to check for updates
    check: "brew outdated"
    # Command to perform updates
    update: "brew upgrade"

  # Simplified format - directly use string value as update command
  npm: "npm update -g"

  # Example for another package manager
  apt:
    check: "apt list --upgradable"
    update: "sudo apt update && sudo apt upgrade"
    
  # Update-only manager (no check command)
  cargo:
    update: "cargo install-update -a"
    
  # Check-only manager (no update command)
  docker-images:
    check: "docker images --format '{{.Repository}}:{{.Tag}}' | grep -v '<none>' | xargs -I{} docker pull {} 2>&1 | grep -v 'up to date'"
```

Two ways to configure package managers:

1. **Standard format**: Specify both `check` and `update` commands as an object
   - `check`: Command to check for updates (optional)
   - `update`: Command to perform the actual update (required for update operations)

2. **Simplified format**: Directly specify a string value, treated as an update command
   ```yaml
   commands:
     npm: "npm update -g"  # Treated as an update command
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
