use crate::scanner::{Tree, TreeIndex};
use petgraph::Direction;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ExtensionStat {
    pub total_bytes: u128,
    pub file_count: u64,
}

#[derive(Debug, Default)]
pub struct ExtensionIndex {
    pub by_ext: HashMap<OsString, ExtensionStat>,
    pub ordered: Vec<OsString>,
}

pub fn build_extension_index(tree: &Tree, root: TreeIndex) -> ExtensionIndex {
    let mut by_ext: HashMap<OsString, ExtensionStat> = HashMap::new();

    collect_extensions(tree, root, &mut by_ext);

    let mut ordered: Vec<_> = by_ext.keys().cloned().collect();
    ordered.sort_by(|a, b| {
        let size_a = by_ext.get(a).map(|s| s.total_bytes).unwrap_or(0);
        let size_b = by_ext.get(b).map(|s| s.total_bytes).unwrap_or(0);
        size_b.cmp(&size_a)
    });

    ExtensionIndex { by_ext, ordered }
}

fn collect_extensions(
    tree: &Tree,
    node: TreeIndex,
    stats: &mut HashMap<OsString, ExtensionStat>,
) {
    let Some(entry) = tree.node_weight(node) else {
        return;
    };

    if entry.is_dir || entry.entry_count.is_some() {
        for child in tree.neighbors_directed(node, Direction::Outgoing) {
            collect_extensions(tree, child, stats);
        }
    } else {
        let ext = get_extension(&entry.name);
        let stat = stats.entry(ext).or_insert(ExtensionStat {
            total_bytes: 0,
            file_count: 0,
        });
        stat.total_bytes += entry.size;
        stat.file_count += 1;
    }
}

fn get_extension(path: &Path) -> OsString {
    path.extension()
        .map(|e| e.to_ascii_lowercase().into())
        .unwrap_or_default()
}
