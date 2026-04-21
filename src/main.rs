#![windows_subsystem = "windows"]

use anyhow::Result;
use clap::{Parser, Subcommand};
use rustdirstat::{
    build_extension_index, get_palette, render_cushions, squarify, BackgroundTraversal,
    CushionOptions, Rect, RustDirStatApp, Traversal, WalkOptions, FALLBACK_COLOR,
};
use std::path::PathBuf;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser)]
#[command(name = "rustdirstat", version, about = "WinDirStat-style disk analyzer")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch the GUI (default)
    Gui,
    /// Scan a directory and print statistics
    Scan {
        /// Path to scan
        path: PathBuf,
        /// Number of top extensions to show
        #[arg(short, long, default_value = "20")]
        top: usize,
    },
    /// Render a treemap to PNG
    Render {
        /// Path to scan
        path: PathBuf,
        /// Output PNG file
        #[arg(short, long, default_value = "treemap.png")]
        output: PathBuf,
        /// Image width
        #[arg(long, default_value = "1024")]
        width: u32,
        /// Image height
        #[arg(long, default_value = "768")]
        height: u32,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Gui) {
        Commands::Gui => run_gui(),
        Commands::Scan { path, top } => run_scan(&path, top),
        Commands::Render { path, output, width, height } => run_render(&path, &output, width, height),
    }
}

fn run_gui() -> Result<()> {
    let icon = load_icon();

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 800.0])
        .with_title("RustDirStat");

    if let Some(icon_data) = icon {
        viewport = viewport.with_icon(std::sync::Arc::new(icon_data));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "RustDirStat",
        options,
        Box::new(|cc| Ok(Box::new(RustDirStatApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("GUI error: {}", e))
}

fn load_icon() -> Option<eframe::egui::IconData> {
    let icon_bytes = include_bytes!("../RustDirStat.png");
    let image = image::load_from_memory(icon_bytes).ok()?.into_rgba8();

    Some(eframe::egui::IconData {
        rgba: image.to_vec(),
        width: image.width(),
        height: image.height(),
    })
}

fn run_scan(path: &PathBuf, top_n: usize) -> Result<()> {
    let path = std::fs::canonicalize(path)?;
    println!("Scanning: {}", path.display());

    let (traversal, stats) = scan_directory(&path)?;

    println!("\nScan complete:");
    println!("  Total size: {} bytes", stats.total_bytes.unwrap_or(0));
    println!("  Entries: {}", stats.entries_traversed);
    println!("  IO errors: {}", stats.io_errors);
    if let Some(elapsed) = stats.elapsed {
        println!("  Time: {:.2}s", elapsed.as_secs_f64());
    }

    let ext_index = build_extension_index(&traversal.tree, traversal.root_index);

    println!("\nTop {} extensions by size:", top_n);
    for ext in ext_index.ordered.iter().take(top_n) {
        if let Some(stat) = ext_index.by_ext.get(ext) {
            let ext_str = ext.to_string_lossy();
            let ext_display = if ext_str.is_empty() {
                "(no ext)"
            } else {
                &ext_str
            };
            println!(
                "  {:<12} {:>15} bytes  ({} files)",
                ext_display, stat.total_bytes, stat.file_count
            );
        }
    }

    Ok(())
}

fn run_render(path: &PathBuf, output: &PathBuf, width: u32, height: u32) -> Result<()> {
    let path = std::fs::canonicalize(path)?;
    println!("Scanning: {}", path.display());

    let (traversal, stats) = scan_directory(&path)?;

    println!(
        "Scan complete: {} entries, {:.2}s",
        stats.entries_traversed,
        stats.elapsed.map(|d| d.as_secs_f64()).unwrap_or(0.0)
    );

    let ext_index = build_extension_index(&traversal.tree, traversal.root_index);
    let palette = get_palette();

    println!("Computing treemap layout...");
    let bounds = Rect::new(0.0, 0.0, width as f64, height as f64);
    let tiles = squarify(&traversal.tree, traversal.root_index, bounds);
    println!("  {} tiles", tiles.len());

    println!("Rendering cushions...");
    let options = CushionOptions::default();

    let pixels = render_cushions(
        &tiles,
        |tile_idx| {
            let node = tiles[tile_idx].node;
            if let Some(entry) = traversal.tree.node_weight(node) {
                if let Some(ext) = entry.name.extension() {
                    let ext_lower = ext.to_ascii_lowercase();
                    if let Some(pos) = ext_index
                        .ordered
                        .iter()
                        .position(|e| e.to_ascii_lowercase() == ext_lower)
                    {
                        if pos < palette.len() {
                            return palette[pos];
                        }
                    }
                }
            }
            FALLBACK_COLOR
        },
        width,
        height,
        &options,
    );

    println!("Saving to {}...", output.display());
    let img = image::RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;
    img.save(output)?;

    println!("Done!");
    Ok(())
}

fn scan_directory(path: &PathBuf) -> Result<(Traversal, rustdirstat::TraversalStats)> {
    let opts = WalkOptions::default();
    let mut traversal = Traversal::new();

    let mut bg = BackgroundTraversal::start(
        traversal.root_index,
        &opts,
        vec![path.clone()],
        false,
        true,
    )?;

    loop {
        match bg.event_rx.recv() {
            Ok(event) => {
                if let Some(finished) = bg.integrate_traversal_event(&mut traversal, event) {
                    if finished {
                        break;
                    }
                }
            }
            Err(_) => break,
        }
    }

    Ok((traversal, bg.stats))
}
