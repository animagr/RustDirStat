#![forbid(unsafe_code)]

pub mod fsops;
pub mod gui;
pub mod model;
pub mod scanner;
pub mod treemap;

pub use model::extension_index::{build_extension_index, ExtensionIndex, ExtensionStat};
pub use scanner::{
    BackgroundTraversal, EntryData, Traversal, TraversalEvent, TraversalStats, Tree, TreeIndex,
    WalkOptions,
};
pub use treemap::{
    get_palette, hit_test, render_cushions, squarify, CushionOptions, Rect, Tile, FALLBACK_COLOR,
};
pub use gui::RustDirStatApp;
