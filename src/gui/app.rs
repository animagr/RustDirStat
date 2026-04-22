use crate::fsops::{delete_recursive, trash_path};
use crate::model::extension_index::{build_extension_index, ExtensionIndex};
use crate::scanner::{BackgroundTraversal, Traversal, TraversalStats, TreeIndex, WalkOptions};
use crate::treemap::{get_palette, hit_test, render_cushions, squarify, CushionOptions, Rect, Tile};
use crate::FALLBACK_COLOR;

use eframe::egui;
use petgraph::Direction;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
enum PendingAction {
    Trash(PathBuf, TreeIndex),
    Delete(PathBuf, TreeIndex),
}

const TREEMAP_UPDATE_INTERVAL: Duration = Duration::from_millis(100);
const LEFT_PANEL_WIDTH: f32 = 300.0;
const LEFT_PANEL_MAX_WIDTH: f32 = 500.0;
const RIGHT_PANEL_WIDTH: f32 = 200.0;

#[derive(Debug, Default)]
enum ScanState {
    #[default]
    Idle,
    Scanning {
        bg: BackgroundTraversal,
        traversal: Traversal,
        last_update: Instant,
    },
    PartialRescan {
        bg: BackgroundTraversal,
        partial_traversal: Traversal,
        main_traversal: Traversal,
        target_node: TreeIndex,
        ext_index: ExtensionIndex,
        last_update: Instant,
    },
    Complete {
        traversal: Traversal,
        #[allow(dead_code)]
        stats: TraversalStats,
        ext_index: ExtensionIndex,
    },
}

#[allow(missing_debug_implementations)]
pub struct RustDirStatApp {
    scan_state: ScanState,
    current_path: Option<PathBuf>,
    treemap_texture: Option<egui::TextureHandle>,
    tiles: Vec<Tile>,
    palette: Vec<[u8; 3]>,
    last_size: (u32, u32),
    status_message: String,
    selected_node: Option<TreeIndex>,
    prev_selected_node: Option<TreeIndex>,
    expanded_nodes: HashSet<TreeIndex>,
    hovered_node: Option<TreeIndex>,
    pending_action: Option<PendingAction>,
    show_confirm_dialog: bool,
    show_about_dialog: bool,
}

impl Default for RustDirStatApp {
    fn default() -> Self {
        Self {
            scan_state: ScanState::Idle,
            current_path: None,
            treemap_texture: None,
            tiles: Vec::new(),
            palette: get_palette(),
            last_size: (0, 0),
            status_message: String::from("Ready. Click 'Open Folder' to scan a directory."),
            selected_node: None,
            prev_selected_node: None,
            expanded_nodes: HashSet::new(),
            hovered_node: None,
            pending_action: None,
            show_confirm_dialog: false,
            show_about_dialog: false,
        }
    }
}

impl RustDirStatApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    fn start_scan(&mut self, path: PathBuf) {
        let opts = WalkOptions::default();
        let traversal = Traversal::new();

        match BackgroundTraversal::start(traversal.root_index, &opts, vec![path.clone()], false, true)
        {
            Ok(bg) => {
                self.current_path = Some(path);
                self.treemap_texture = None;
                self.tiles.clear();
                self.selected_node = None;
                self.expanded_nodes.clear();
                self.scan_state = ScanState::Scanning {
                    bg,
                    traversal,
                    last_update: Instant::now(),
                };
                self.status_message = String::from("Scanning...");
            }
            Err(e) => {
                self.status_message = format!("Failed to start scan: {}", e);
            }
        }
    }

    fn start_partial_rescan(&mut self, path: PathBuf, target_node: TreeIndex) {
        let (main_traversal, ext_index) = match std::mem::take(&mut self.scan_state) {
            ScanState::Complete { traversal, ext_index, .. } => (traversal, ext_index),
            other => {
                self.scan_state = other;
                return;
            }
        };

        let opts = WalkOptions::default();
        let partial_traversal = Traversal::new();

        match BackgroundTraversal::start(
            partial_traversal.root_index,
            &opts,
            vec![path.clone()],
            false,
            true,
        ) {
            Ok(bg) => {
                self.status_message = format!("Rescanning: {}", path.display());
                self.scan_state = ScanState::PartialRescan {
                    bg,
                    partial_traversal,
                    main_traversal,
                    target_node,
                    ext_index,
                    last_update: Instant::now(),
                };
            }
            Err(e) => {
                self.status_message = format!("Failed to start rescan: {}", e);
                self.scan_state = ScanState::Complete {
                    traversal: main_traversal,
                    stats: TraversalStats::default(),
                    ext_index,
                };
            }
        }
    }

    fn process_scan_events(&mut self, ctx: &egui::Context) {
        self.process_full_scan_events(ctx);
        self.process_partial_scan_events(ctx);
    }

    fn process_full_scan_events(&mut self, ctx: &egui::Context) {
        let should_finish = match &mut self.scan_state {
            ScanState::Scanning {
                bg,
                traversal,
                last_update,
            } => {
                let mut finished = false;
                let mut needs_repaint = false;

                while let Ok(event) = bg.event_rx.try_recv() {
                    if let Some(done) = bg.integrate_traversal_event(traversal, event) {
                        if done {
                            finished = true;
                            break;
                        }
                        needs_repaint = true;
                    }
                }

                if !finished {
                    self.status_message = format!(
                        "Scanning... {} entries, {} errors",
                        bg.stats.entries_traversed, bg.stats.io_errors
                    );

                    if needs_repaint && last_update.elapsed() >= TREEMAP_UPDATE_INTERVAL {
                        *last_update = Instant::now();
                    }

                    ctx.request_repaint_after(Duration::from_millis(50));
                }

                finished
            }
            _ => false,
        };

        if should_finish {
            if let ScanState::Scanning { bg, traversal, .. } =
                std::mem::replace(&mut self.scan_state, ScanState::Idle)
            {
                let stats = bg.stats;
                let ext_index = build_extension_index(&traversal.tree, traversal.root_index);

                self.status_message = format!(
                    "Complete: {} entries, {} bytes, {:.2}s",
                    stats.entries_traversed,
                    stats.total_bytes.unwrap_or(0),
                    stats.elapsed.map(|d| d.as_secs_f64()).unwrap_or(0.0)
                );

                self.treemap_texture = None;
                self.expanded_nodes.insert(traversal.root_index);

                self.scan_state = ScanState::Complete {
                    traversal,
                    stats,
                    ext_index,
                };
            }
        }
    }

    fn process_partial_scan_events(&mut self, ctx: &egui::Context) {
        let should_finish = match &mut self.scan_state {
            ScanState::PartialRescan {
                bg,
                partial_traversal,
                last_update,
                ..
            } => {
                let mut finished = false;

                while let Ok(event) = bg.event_rx.try_recv() {
                    if let Some(done) = bg.integrate_traversal_event(partial_traversal, event) {
                        if done {
                            finished = true;
                            break;
                        }
                    }
                }

                if !finished {
                    self.status_message = format!(
                        "Rescanning... {} entries",
                        bg.stats.entries_traversed
                    );

                    if last_update.elapsed() >= TREEMAP_UPDATE_INTERVAL {
                        *last_update = Instant::now();
                    }

                    ctx.request_repaint_after(Duration::from_millis(50));
                }

                finished
            }
            _ => false,
        };

        if should_finish {
            if let ScanState::PartialRescan {
                bg,
                partial_traversal,
                mut main_traversal,
                target_node,
                ..
            } = std::mem::replace(&mut self.scan_state, ScanState::Idle)
            {
                self.merge_partial_scan(
                    &mut main_traversal,
                    target_node,
                    partial_traversal,
                );

                let ext_index = build_extension_index(&main_traversal.tree, main_traversal.root_index);

                self.status_message = format!(
                    "Rescan complete: {} entries, {:.2}s",
                    bg.stats.entries_traversed,
                    bg.stats.elapsed.map(|d| d.as_secs_f64()).unwrap_or(0.0)
                );

                self.treemap_texture = None;

                self.scan_state = ScanState::Complete {
                    traversal: main_traversal,
                    stats: bg.stats,
                    ext_index,
                };
            }
        }
    }

    fn merge_partial_scan(
        &mut self,
        main: &mut Traversal,
        target_node: TreeIndex,
        partial: Traversal,
    ) {
        let old_size = main.tree.node_weight(target_node).map(|e| e.size).unwrap_or(0);

        let children: Vec<_> = main.tree
            .neighbors_directed(target_node, Direction::Outgoing)
            .collect();
        for child in children {
            fn remove_subtree(tree: &mut crate::scanner::Tree, node: TreeIndex) {
                let children: Vec<_> = tree.neighbors_directed(node, Direction::Outgoing).collect();
                for c in children {
                    remove_subtree(tree, c);
                }
                tree.remove_node(node);
            }
            remove_subtree(&mut main.tree, child);
        }

        let partial_root = partial.root_index;
        let new_size = partial.tree.node_weight(partial_root).map(|e| e.size).unwrap_or(0);

        let partial_children: Vec<_> = partial.tree
            .neighbors_directed(partial_root, Direction::Outgoing)
            .collect();

        let mut node_map = std::collections::HashMap::new();

        fn copy_subtree(
            src: &crate::scanner::Tree,
            dst: &mut crate::scanner::Tree,
            src_node: TreeIndex,
            dst_parent: TreeIndex,
            node_map: &mut std::collections::HashMap<TreeIndex, TreeIndex>,
        ) {
            if let Some(entry) = src.node_weight(src_node) {
                let new_node = dst.add_node(entry.clone());
                dst.add_edge(dst_parent, new_node, ());
                node_map.insert(src_node, new_node);

                let children: Vec<_> = src.neighbors_directed(src_node, Direction::Outgoing).collect();
                for child in children {
                    copy_subtree(src, dst, child, new_node, node_map);
                }
            }
        }

        for child in partial_children {
            copy_subtree(&partial.tree, &mut main.tree, child, target_node, &mut node_map);
        }

        if let Some(entry) = main.tree.node_weight_mut(target_node) {
            entry.size = new_size;
        }

        let size_diff = new_size as i128 - old_size as i128;
        let mut current = target_node;
        while let Some(parent) = main.tree.neighbors_directed(current, Direction::Incoming).next() {
            if let Some(entry) = main.tree.node_weight_mut(parent) {
                entry.size = ((entry.size as i128) + size_diff).max(0) as u128;
            }
            current = parent;
        }

        self.expanded_nodes.insert(target_node);
    }

    fn render_treemap(&mut self, ctx: &egui::Context, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        let (traversal, ext_index) = match &self.scan_state {
            ScanState::Complete {
                traversal,
                ext_index,
                ..
            } => (traversal, ext_index),
            ScanState::PartialRescan { main_traversal, ext_index, .. } => (main_traversal, ext_index),
            ScanState::Scanning { traversal, .. } => {
                let ext_index = build_extension_index(&traversal.tree, traversal.root_index);
                let bounds = Rect::new(0.0, 0.0, width as f64, height as f64);
                self.tiles = squarify(&traversal.tree, traversal.root_index, bounds);

                let options = CushionOptions::default();
                let palette = &self.palette;

                let pixels = render_cushions(
                    &self.tiles,
                    |tile_idx| tile_color(&self.tiles, tile_idx, traversal, &ext_index, palette),
                    width,
                    height,
                    &options,
                );

                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [width as usize, height as usize],
                    &pixels,
                );
                self.treemap_texture =
                    Some(ctx.load_texture("treemap", image, egui::TextureOptions::default()));
                self.last_size = (width, height);
                return;
            }
            _ => return,
        };

        let size_changed = self.last_size != (width, height);
        let selection_changed = self.selected_node != self.prev_selected_node;
        let need_render = self.treemap_texture.is_none() || size_changed || selection_changed;

        if !need_render {
            return;
        }

        self.prev_selected_node = self.selected_node;

        let bounds = Rect::new(0.0, 0.0, width as f64, height as f64);
        self.tiles = squarify(&traversal.tree, traversal.root_index, bounds);

        let options = CushionOptions::default();
        let palette = &self.palette;

        let mut pixels = render_cushions(
            &self.tiles,
            |tile_idx| tile_color(&self.tiles, tile_idx, traversal, ext_index, palette),
            width,
            height,
            &options,
        );

        if let Some(selected) = self.selected_node {
            draw_selection_highlight(&mut pixels, width, height, &self.tiles, selected);
        }

        let image =
            egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &pixels);
        self.treemap_texture =
            Some(ctx.load_texture("treemap", image, egui::TextureOptions::default()));
        self.last_size = (width, height);
    }

    fn show_tree_panel(&mut self, ui: &mut egui::Ui) {
        let traversal = match &self.scan_state {
            ScanState::Complete { traversal, .. } => traversal,
            ScanState::PartialRescan { main_traversal, .. } => main_traversal,
            _ => return,
        };

        let root = traversal.root_index;
        let tree_data = collect_tree_data(traversal, root, &self.expanded_nodes);

        let mut clicks = Vec::new();
        let mut toggle_expand = Vec::new();
        let mut context_actions: Vec<(TreeIndex, &str)> = Vec::new();

        egui::ScrollArea::both().show(ui, |ui| {
            for item in &tree_data {
                ui.horizontal(|ui| {
                    ui.add_space(item.depth as f32 * 16.0);

                    if item.has_children && item.is_dir {
                        let icon = if item.is_expanded { "▼" } else { "▶" };
                        if ui.small_button(icon).clicked() {
                            toggle_expand.push(item.node);
                        }
                    } else {
                        ui.add_space(18.0);
                    }

                    let is_selected = self.selected_node == Some(item.node);
                    let response = ui.selectable_label(is_selected, &item.label);

                    if response.clicked() {
                        clicks.push(item.node);
                    }

                    response.context_menu(|ui| {
                        if ui.button("Open in Explorer").clicked() {
                            context_actions.push((item.node, "explorer"));
                            ui.close_menu();
                        }
                        ui.separator();
                        if item.is_dir {
                            if ui.button("Rescan Folder").clicked() {
                                context_actions.push((item.node, "rescan"));
                                ui.close_menu();
                            }
                            ui.separator();
                        }
                        if ui.button("Move to Trash").clicked() {
                            context_actions.push((item.node, "trash"));
                            ui.close_menu();
                        }
                        if ui.button("Delete Permanently").clicked() {
                            context_actions.push((item.node, "delete"));
                            ui.close_menu();
                        }
                    });
                });
            }
        });

        for node in toggle_expand {
            if self.expanded_nodes.contains(&node) {
                self.expanded_nodes.remove(&node);
            } else {
                self.expanded_nodes.insert(node);
            }
        }

        if let Some(node) = clicks.into_iter().last() {
            self.selected_node = Some(node);
        }

        for (node, action) in context_actions {
            if let Some(path) = self.get_node_full_path(node) {
                match action {
                    "trash" => {
                        self.pending_action = Some(PendingAction::Trash(path, node));
                        self.show_confirm_dialog = true;
                    }
                    "delete" => {
                        self.pending_action = Some(PendingAction::Delete(path, node));
                        self.show_confirm_dialog = true;
                    }
                    "rescan" => {
                        self.start_partial_rescan(path, node);
                    }
                    "explorer" => {
                        #[cfg(target_os = "windows")]
                        {
                            let _ = std::process::Command::new("explorer.exe")
                                .arg("/select,")
                                .arg(&path)
                                .spawn();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn get_node_full_path(&self, node: TreeIndex) -> Option<PathBuf> {
        let traversal = match &self.scan_state {
            ScanState::Complete { traversal, .. } => traversal,
            ScanState::PartialRescan { main_traversal, .. } => main_traversal,
            _ => return None,
        };

        let mut path_parts = Vec::new();
        let mut current = node;

        loop {
            if let Some(entry) = traversal.tree.node_weight(current) {
                path_parts.push(entry.name.clone());
            }
            match traversal.tree.neighbors_directed(current, Direction::Incoming).next() {
                Some(p) => current = p,
                None => break,
            }
        }

        path_parts.reverse();
        if path_parts.is_empty() {
            None
        } else {
            let mut full_path = PathBuf::new();
            for part in path_parts {
                if full_path.as_os_str().is_empty() {
                    full_path = part;
                } else {
                    full_path = full_path.join(part.file_name().unwrap_or(part.as_os_str()));
                }
            }
            Some(full_path)
        }
    }

    fn show_confirm_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_confirm_dialog {
            return;
        }

        let action = match &self.pending_action {
            Some(a) => a.clone(),
            None => return,
        };

        let (title, message, path, node) = match &action {
            PendingAction::Trash(p, n) => (
                "Move to Trash",
                format!("Move to trash?\n\n{}", p.display()),
                p.clone(),
                *n,
            ),
            PendingAction::Delete(p, n) => (
                "Delete Permanently",
                format!("Permanently delete? This cannot be undone!\n\n{}", p.display()),
                p.clone(),
                *n,
            ),
        };

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(message);
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.show_confirm_dialog = false;
                        self.pending_action = None;
                    }

                    let confirm_text = match &action {
                        PendingAction::Trash(_, _) => "Move to Trash",
                        PendingAction::Delete(_, _) => "Delete",
                    };

                    if ui.button(confirm_text).clicked() {
                        self.execute_pending_action(&path, &action, node);
                        self.show_confirm_dialog = false;
                        self.pending_action = None;
                    }
                });
            });
    }

    fn show_about_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_about_dialog {
            return;
        }

        egui::Window::new("About RustDirStat")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.heading("RustDirStat - Directory Statistics");
                ui.add_space(10.0);
                ui.label("\"Shows where all your disk space has gone");
                ui.label("and helps you clean it up.\"");
                ui.add_space(10.0);
                ui.label("Based on Byron's dua-cli");
                ui.hyperlink("https://github.com/Byron/dua-cli");
                ui.add_space(10.0);
                ui.label("RustDirStat's home:");
                ui.hyperlink("https://github.com/animagr/rustdirstat");
                ui.add_space(10.0);
                ui.label("Copyright (c) 2026 by animagr");
                ui.add_space(15.0);

                if ui.button("Close").clicked() {
                    self.show_about_dialog = false;
                }
            });
    }

    fn execute_pending_action(&mut self, path: &PathBuf, action: &PendingAction, node: TreeIndex) {
        let result = match action {
            PendingAction::Trash(..) => trash_path(path).map(|_| "Moved to trash"),
            PendingAction::Delete(..) => delete_recursive(path).map(|n| {
                if n == 1 { "Deleted 1 item" } else { "Deleted items" }
            }),
        };

        match result {
            Ok(msg) => {
                self.status_message = format!("{}: {}", msg, path.display());
                self.remove_node_from_tree(node);
            }
            Err(e) => {
                self.status_message = format!("Error: {}", e);
            }
        }
    }

    fn remove_node_from_tree(&mut self, node: TreeIndex) {
        let (traversal, old_ext_index) = match &mut self.scan_state {
            ScanState::Complete { traversal, ext_index, .. } => (traversal, ext_index),
            _ => return,
        };

        let node_size = traversal.tree.node_weight(node).map(|e| e.size).unwrap_or(0);

        let mut ancestors = Vec::new();
        let mut current = node;
        while let Some(parent) = traversal.tree.neighbors_directed(current, Direction::Incoming).next() {
            ancestors.push(parent);
            current = parent;
        }

        fn remove_subtree(tree: &mut crate::scanner::Tree, node: TreeIndex) {
            let children: Vec<_> = tree.neighbors_directed(node, Direction::Outgoing).collect();
            for child in children {
                remove_subtree(tree, child);
            }
            tree.remove_node(node);
        }
        remove_subtree(&mut traversal.tree, node);

        for ancestor in &ancestors {
            if let Some(entry) = traversal.tree.node_weight_mut(*ancestor) {
                entry.size = entry.size.saturating_sub(node_size);
                if let Some(count) = entry.entry_count.as_mut() {
                    *count = count.saturating_sub(1);
                }
            }
        }

        *old_ext_index = build_extension_index(&traversal.tree, traversal.root_index);

        if self.selected_node == Some(node) {
            self.selected_node = None;
        }
        self.expanded_nodes.remove(&node);
        self.treemap_texture = None;
    }

    fn show_legend_panel(&mut self, ui: &mut egui::Ui) {
        let ext_index = match &self.scan_state {
            ScanState::Complete { ext_index, .. } => ext_index,
            ScanState::PartialRescan { ext_index, .. } => ext_index,
            _ => return,
        };

        ui.heading("Extensions");
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, ext) in ext_index.ordered.iter().take(self.palette.len()).enumerate() {
                let color = self.palette[i];
                let stat = ext_index.by_ext.get(ext);

                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(16.0, 16.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(
                        rect,
                        0.0,
                        egui::Color32::from_rgb(color[0], color[1], color[2]),
                    );

                    let ext_str = ext.to_string_lossy();
                    let ext_display = if ext_str.is_empty() { "(none)" } else { &ext_str };

                    if let Some(stat) = stat {
                        ui.label(format!("{}: {}", ext_display, format_size(stat.total_bytes)));
                    } else {
                        ui.label(ext_display.to_string());
                    }
                });
            }
        });
    }

    fn handle_treemap_click(&mut self, pos: egui::Pos2, rect: egui::Rect) {
        let x = (pos.x - rect.left()) as f64;
        let y = (pos.y - rect.top()) as f64;

        if let Some(node) = hit_test(&self.tiles, x, y) {
            self.selected_node = Some(node);
            self.expand_to_node(node);
        }
    }

    fn expand_to_node(&mut self, node: TreeIndex) {
        let traversal = match &self.scan_state {
            ScanState::Complete { traversal, .. } => traversal,
            _ => return,
        };

        let mut current = node;
        loop {
            let parent = traversal
                .tree
                .neighbors_directed(current, Direction::Incoming)
                .next();
            match parent {
                Some(p) => {
                    self.expanded_nodes.insert(p);
                    current = p;
                }
                None => break,
            }
        }
    }

    fn get_hovered_path(&self) -> Option<PathBuf> {
        let traversal = match &self.scan_state {
            ScanState::Complete { traversal, .. } => traversal,
            ScanState::PartialRescan { main_traversal, .. } => main_traversal,
            _ => return None,
        };

        let node = self.hovered_node?;
        let mut path_parts = Vec::new();
        let mut current = node;

        loop {
            if let Some(entry) = traversal.tree.node_weight(current) {
                path_parts.push(entry.name.clone());
            }
            match traversal.tree.neighbors_directed(current, Direction::Incoming).next() {
                Some(p) => current = p,
                None => break,
            }
        }

        path_parts.reverse();
        if path_parts.is_empty() {
            None
        } else {
            let mut full_path = PathBuf::new();
            for part in path_parts {
                if full_path.as_os_str().is_empty() {
                    full_path = part;
                } else {
                    full_path = full_path.join(part.file_name().unwrap_or(part.as_os_str()));
                }
            }
            Some(full_path)
        }
    }
}

struct TreeItem {
    node: TreeIndex,
    label: String,
    depth: usize,
    is_dir: bool,
    is_expanded: bool,
    has_children: bool,
}

fn collect_tree_data(
    traversal: &Traversal,
    root: TreeIndex,
    expanded: &HashSet<TreeIndex>,
) -> Vec<TreeItem> {
    let mut items = Vec::new();
    collect_tree_node(traversal, root, 0, expanded, &mut items);
    items
}

fn collect_tree_node(
    traversal: &Traversal,
    node: TreeIndex,
    depth: usize,
    expanded: &HashSet<TreeIndex>,
    items: &mut Vec<TreeItem>,
) {
    let entry = match traversal.tree.node_weight(node) {
        Some(e) => e,
        None => return,
    };

    let is_dir = entry.is_dir || entry.entry_count.is_some();
    let is_expanded = expanded.contains(&node);

    let children: Vec<_> = traversal
        .tree
        .neighbors_directed(node, Direction::Outgoing)
        .collect();
    let has_children = !children.is_empty();

    let name = entry
        .name
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| entry.name.to_string_lossy().to_string());

    let label = format!("{} ({})", name, format_size(entry.size));

    items.push(TreeItem {
        node,
        label,
        depth,
        is_dir,
        is_expanded,
        has_children,
    });

    if is_expanded && has_children {
        let mut sorted_children: Vec<_> = children
            .into_iter()
            .filter_map(|idx| traversal.tree.node_weight(idx).map(|e| (idx, e.size)))
            .collect();
        sorted_children.sort_by(|a, b| b.1.cmp(&a.1));

        for (child_idx, _) in sorted_children {
            collect_tree_node(traversal, child_idx, depth + 1, expanded, items);
        }
    }
}

fn draw_selection_highlight(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    tiles: &[Tile],
    selected: TreeIndex,
) {
    let tile = tiles.iter().find(|t| t.node == selected);
    let tile = match tile {
        Some(t) => t,
        None => return,
    };

    let x1 = (tile.rect.x as u32).min(width.saturating_sub(1));
    let y1 = (tile.rect.y as u32).min(height.saturating_sub(1));
    let x2 = ((tile.rect.x + tile.rect.width) as u32).min(width);
    let y2 = ((tile.rect.y + tile.rect.height) as u32).min(height);

    let highlight_color = [255u8, 255, 255, 255];
    let border_width = 2u32;

    for x in x1..x2 {
        for b in 0..border_width {
            if y1 + b < height {
                let idx = ((y1 + b) * width + x) as usize * 4;
                if idx + 3 < pixels.len() {
                    pixels[idx..idx + 4].copy_from_slice(&highlight_color);
                }
            }
            if y2 > b && y2 - 1 - b < height {
                let idx = ((y2 - 1 - b) * width + x) as usize * 4;
                if idx + 3 < pixels.len() {
                    pixels[idx..idx + 4].copy_from_slice(&highlight_color);
                }
            }
        }
    }

    for y in y1..y2 {
        for b in 0..border_width {
            if x1 + b < width {
                let idx = (y * width + x1 + b) as usize * 4;
                if idx + 3 < pixels.len() {
                    pixels[idx..idx + 4].copy_from_slice(&highlight_color);
                }
            }
            if x2 > b && x2 - 1 - b < width {
                let idx = (y * width + x2 - 1 - b) as usize * 4;
                if idx + 3 < pixels.len() {
                    pixels[idx..idx + 4].copy_from_slice(&highlight_color);
                }
            }
        }
    }
}

fn tile_color(
    tiles: &[Tile],
    tile_idx: usize,
    traversal: &Traversal,
    ext_index: &ExtensionIndex,
    palette: &[[u8; 3]],
) -> [u8; 3] {
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
}

fn format_size(bytes: u128) -> String {
    const KB: u128 = 1024;
    const MB: u128 = KB * 1024;
    const GB: u128 = MB * 1024;
    const TB: u128 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

impl eframe::App for RustDirStatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_scan_events(ctx);
        self.show_confirm_dialog(ctx);
        self.show_about_dialog(ctx);

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open Folder").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.start_scan(path);
                    }
                }

                if matches!(self.scan_state, ScanState::Complete { .. }) {
                    if ui.button("Rescan").clicked() {
                        if let Some(path) = self.current_path.clone() {
                            self.start_scan(path);
                        }
                    }
                }

                if ui.button("About").clicked() {
                    self.show_about_dialog = true;
                }

                ui.separator();

                if let Some(path) = &self.current_path {
                    ui.label(path.display().to_string());
                }
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);

                if let Some(path) = self.get_hovered_path() {
                    ui.separator();
                    ui.label(path.display().to_string());
                }
            });
        });

        egui::SidePanel::left("tree_panel")
            .default_width(LEFT_PANEL_WIDTH)
            .max_width(LEFT_PANEL_MAX_WIDTH)
            .show(ctx, |ui| {
                ui.heading("Files");
                ui.separator();
                self.show_tree_panel(ui);
            });

        egui::SidePanel::right("legend_panel")
            .default_width(RIGHT_PANEL_WIDTH)
            .show(ctx, |ui| {
                self.show_legend_panel(ui);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let available = ui.available_size();
            let width = available.x as u32;
            let height = available.y as u32;

            if width > 0 && height > 0 {
                self.render_treemap(ctx, width, height);

                if let Some(texture) = &self.treemap_texture {
                    let image_rect = ui.available_rect_before_wrap();

                    let image = egui::Image::new(egui::load::SizedTexture::new(
                        texture.id(),
                        egui::vec2(width as f32, height as f32),
                    ))
                    .sense(egui::Sense::click());

                    let response = ui.add(image);

                    if response.clicked() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            self.handle_treemap_click(pos, image_rect);
                        }
                    }

                    if response.hovered() {
                        if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                            let x = (pos.x - image_rect.left()) as f64;
                            let y = (pos.y - image_rect.top()) as f64;
                            self.hovered_node = hit_test(&self.tiles, x, y);
                        }
                    } else {
                        self.hovered_node = None;
                    }
                }
            }
        });
    }
}
