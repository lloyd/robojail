# RoboJail

Sandboxed development environments for AI coding assistants on Arch Linux.

RoboJail creates isolated workspaces where AI tools (Claude Code, Codex, etc.) can safely modify code without affecting your host system. Each jail is a git worktree with read-only access to system binaries and full read-write access to project files.

## Features

- **Pure Rust** - No external dependencies (no bubblewrap, no Docker)
- **Git-integrated** - Each jail is a git worktree, making it easy to review and merge AI changes
- **Namespace isolation** - Uses Linux user/mount/IPC namespaces for security
- **External supervision** - Monitor AI progress from outside the jail with `robojail status`
- **Fast** - Jails start instantly (no container image layers)
- **Simple** - Sensible defaults, minimal configuration needed

## Requirements

- Arch Linux (or any Linux with kernel 5.0+)
- User namespaces enabled (`/proc/sys/kernel/unprivileged_userns_clone = 1`)
- Git

## Installation

```bash
# From source
git clone https://github.com/robojail/robojail
cd robojail
cargo install --path .

# Or build locally
cargo build --release
sudo cp target/release/robojail /usr/local/bin/
```

## Quick Start

```bash
# Create a jail from your project
robojail create --name ai-task --repo ~/projects/myapp

# Enter the jail interactively
robojail enter ai-task

# Inside the jail:
# - / is your project root (read-write)
# - /usr, /bin, /lib are system dirs (read-only)
# - You're root inside, but actually your user outside

# Run a command in the jail
robojail run ai-task -- cargo build

# Check what the AI has modified (from outside)
robojail status ai-task
robojail status ai-task --diff
robojail status ai-task --json

# List all jails
robojail list

# Clean up
robojail destroy ai-task
```

## Command Reference

### `robojail create`

Create a new jail from a git repository.

```bash
robojail create --name <name> --repo <path> [--branch <branch>]
```

- `--name` - Unique name for the jail (alphanumeric, dash, underscore)
- `--repo` - Path to the git repository
- `--branch` - Base branch for the worktree (default: HEAD)

Creates a git worktree at `~/.local/share/robojail/jails/<name>/`.

### `robojail list`

List all jails.

```bash
robojail list [--json]
```

### `robojail enter`

Enter a jail interactively.

```bash
robojail enter <name>
```

Drops you into a shell inside the sandboxed environment.

### `robojail run`

Run a command inside a jail.

```bash
robojail run <name> -- <command> [args...]
```

Example: `robojail run ai-task -- cargo test`

### `robojail status`

Show git status of a jail (external supervisor view).

```bash
robojail status <name> [--json] [--diff]
```

This runs git commands from OUTSIDE the jail, allowing you to monitor AI progress without entering the sandbox.

### `robojail destroy`

Destroy a jail and clean up its worktree.

```bash
robojail destroy <name> [--force]
```

- `--force` - Destroy even if the jail is running or has unsaved changes

## Security Model

| Resource | Access |
|----------|--------|
| Project files | Read-write |
| `/usr`, `/bin`, `/lib`, `/sbin` | Read-only |
| `/etc` (minimal) | Read-only |
| `/proc` | Read-only (bind-mounted) |
| `/dev` | Minimal devices only |
| `/tmp` | Isolated tmpfs |
| Home directory | Hidden |
| Credentials (`.ssh`, `.gnupg`, etc.) | Hidden |

The sandbox uses:
- **User namespace** - Unprivileged root inside the jail
- **Mount namespace** - Isolated filesystem view
- **IPC namespace** - Isolated inter-process communication
- **UTS namespace** - Isolated hostname

## Configuration

Configuration file: `~/.config/robojail/config.toml`

```toml
# Default shell inside jails
default_shell = "/bin/bash"

# Share network with host (default: true)
network_enabled = true

# Additional read-only bind mounts
extra_ro_binds = []

# Additional read-write bind mounts
extra_rw_binds = []

# Paths in $HOME to hide (relative paths)
hidden_paths = [
    ".ssh",
    ".gnupg",
    ".aws",
    ".config/gcloud",
    ".kube",
    ".docker",
]

# Environment variables to pass through
env_passthrough = ["TERM", "LANG", "LC_ALL", "COLORTERM"]
```

## File Locations

| Purpose | Path |
|---------|------|
| Configuration | `~/.config/robojail/config.toml` |
| Jail data | `~/.local/share/robojail/jails/` |
| State file | `~/.local/state/robojail/jails.json` |

## Troubleshooting

### "user namespaces are not available"

Check that unprivileged user namespaces are enabled:

```bash
cat /proc/sys/kernel/unprivileged_userns_clone
# Should be 1

# If 0, enable temporarily:
sudo sysctl kernel.unprivileged_userns_clone=1

# Or permanently in /etc/sysctl.d/userns.conf:
# kernel.unprivileged_userns_clone = 1
```

### "not a git repository"

RoboJail requires a git repository. Initialize one first:

```bash
cd /path/to/project
git init
git add .
git commit -m "Initial commit"
```

### Jail won't destroy

Use `--force` to destroy running or dirty jails:

```bash
robojail destroy my-jail --force
```

## How It Works

1. **Create**: Creates a git worktree in `~/.local/share/robojail/jails/<name>/`
2. **Enter/Run**:
   - Creates a user namespace (you become root inside, but are still you outside)
   - Creates a mount namespace with:
     - Your project at `/` (read-write)
     - System directories bind-mounted read-only
     - Isolated `/tmp` and `/dev`
   - Applies security restrictions (PR_SET_NO_NEW_PRIVS, new session)
3. **Status**: Runs git commands on the worktree from outside the sandbox
4. **Destroy**: Removes the worktree and cleans up state

## Use with AI Coding Assistants

RoboJail is designed for running AI coding assistants like Claude Code in isolation:

```bash
# Create jail for AI work
robojail create --name claude-task --repo ~/myproject

# Run Claude Code inside the jail
robojail run claude-task -- claude

# Monitor from another terminal
watch robojail status claude-task

# When done, review changes
robojail status claude-task --diff

# If satisfied, merge the worktree branch
cd ~/myproject
git merge robojail/claude-task-abc123

# Clean up
robojail destroy claude-task
```

## License

MIT
