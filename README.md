# tx

A terminal UI for running monorepo commands side by side.

Instead of opening separate terminal tabs for your API, web client, and worker, `tx` runs them concurrently in one window with a clickable sidebar.

```text
┌ Services (dev) ────────┬ apps/backend ──────────────────────────────┐
│ ● apps/backend         │ [14:02:11] Server listening on port 3000   │
│ ● apps/frontend        │ [14:02:12] Ready in 240ms                  │
└────────────────────────┴────────────────────────────────────────────┘
  [↑/↓] Select   [Click] Focus   [r] Restart   [q] Quit
```

## How it works

`tx` reads your workspace configuration (`package.json` workspaces or `pnpm-workspace.yaml`). It finds packages that define the script you asked for, extracts their relative paths (like `apps/backend`), and runs them in parallel.

Packages that lack the script are skipped automatically. There are no configuration files to write or maintain.

Each process runs inside its own pseudo-terminal (PTY). Dev tools like Vite, Next.js, and Bun detect a real terminal, so colors, cursor updates, and live outputs work without workarounds.

## Installation

### macOS and Linux

```bash
curl -fsSL https://raw.githubusercontent.com/cismuc/tx/main/install.sh | sh
```

### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/cismuc/tx/main/install.ps1 | iex
```

### From source

```bash
cargo install --git https://github.com/cismuc/tx.git
```

## Usage

Run dev servers across all workspaces:

```bash
tx
# or explicitly:
tx dev
```

Run other scripts:

```bash
tx test        # runs "test" only in packages that have it
tx build       # runs "build" only in packages that have it
tx typecheck   # runs "typecheck" only in packages that have it
```

Target a specific directory:

```bash
tx dev --dir /path/to/repo
```

## Controls

| Key / Action | What it does |
| :--- | :--- |
| `↑` / `k` | Select previous service |
| `↓` / `j` | Select next service |
| `Left Click` (Sidebar) | Focus clicked service |
| `Mouse Scroll` (Sidebar) | Cycle through services |
| `Mouse Scroll` (Logs) | Scroll through terminal output |
| `r` | Restart active service |
| `R` | Restart all services |
| `q` / `Ctrl+C` | Quit and terminate all processes |
