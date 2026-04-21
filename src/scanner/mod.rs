mod crossdev;
mod inodefilter;
mod traverse;
mod walk;

pub use traverse::{
    BackgroundTraversal, EntryData, Traversal, TraversalEvent, TraversalStats, Tree, TreeIndex,
};
pub use walk::{ByteFormat, TraversalSorting, WalkOptions, WalkResult};

pub(crate) fn get_entry_or_panic(tree: &Tree, node_idx: TreeIndex) -> &EntryData {
    tree.node_weight(node_idx)
        .expect("node should always be retrievable with valid index")
}

pub(crate) fn get_size_or_panic(tree: &Tree, node_idx: TreeIndex) -> u128 {
    get_entry_or_panic(tree, node_idx).size
}
