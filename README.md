# RustDirStat

<p align="center">
  <img src="RustDirStat.png" alt="RustDirStat Icon" width="128" height="128">
</p>

<p align="center">
  <strong>A WinDirStat-style disk space analyzer with cushion treemap visualization</strong>
</p>

<p align="center">
  <em>"Shows where all your disk space has gone and helps you clean it up."</em>
</p>

---

**RustDirStat** is a graphical disk usage analyzer for Windows, inspired by [WinDirStat](https://windirstat.net/). It provides a cushion-shaded treemap visualization that makes it easy to see which files and folders are consuming your disk space.

Built in Rust using [egui](https://github.com/emilk/egui) for the GUI and [jwalk](https://github.com/Byron/jwalk) for fast parallel directory scanning (via [dua-cli](https://github.com/Byron/dua-cli)).

## Features

- **Cushion Treemap Visualization** - Beautiful 3D-shaded treemap like classic WinDirStat
- **Fast Parallel Scanning** - Multi-threaded directory traversal using jwalk
- **File Tree Panel** - Expandable directory tree sorted by size
- **Extension Legend** - Color-coded file type statistics
- **Interactive Selection** - Click treemap tiles or tree items to select; selection syncs between views
- **Hover Path Display** - See full path of hovered items in status bar
- **Delete/Trash Files** - Right-click context menu to move files to trash or delete permanently
- **Rescan Folders** - Right-click to rescan individual subfolders without full rescan
- **In-Memory Updates** - Deletions update the tree instantly without rescanning

## Screenshots

*Coming soon*

## Installation

### Pre-built Binaries

See the [Releases](https://github.com/animagr/rustdirstat/releases) page for pre-built Windows executables.

### Build from Source

Requires [Rust](https://rustup.rs/) 1.75 or later.

```bash
# Clone the repository
git clone https://github.com/animagr/rustdirstat.git
cd rustdirstat

# Debug build
cargo build

# Release build (optimized, smaller binary)
cargo build --release
```

The executable will be in `target/release/rustdirstat.exe`.

## Usage

### GUI Mode (Default)

Simply run the executable:

```bash
rustdirstat
```

Or double-click `rustdirstat.exe` in Windows Explorer.

1. Click **Open Folder** to select a directory to scan
2. Wait for the scan to complete
3. Explore the treemap and file tree
4. Right-click items to delete or rescan

### Command Line

```bash
# Scan a directory and print statistics
rustdirstat scan C:\Users\YourName\Downloads

# Scan with top N extensions (default: 20)
rustdirstat scan C:\Users\YourName\Downloads --top 30

# Render a treemap to PNG
rustdirstat render C:\Users\YourName\Downloads -o treemap.png

# Render with custom dimensions
rustdirstat render C:\Users\YourName\Downloads -o treemap.png --width 1920 --height 1080
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Click treemap tile | Select file/folder |
| Click tree item | Select and highlight in treemap |
| Right-click | Context menu (delete, rescan) |

## Acknowledgments

- **[dua-cli](https://github.com/Byron/dua-cli)** by Byron - Fast parallel directory traversal
- **[WinDirStat](https://windirstat.net/)** - Original inspiration for the cushion treemap visualization
- **[egui](https://github.com/emilk/egui)** - Immediate mode GUI library

## License

GPL-3.0 License

Copyright (c) 2026 animagr

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.
