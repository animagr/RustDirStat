use crate::scanner::{Tree, TreeIndex};
use petgraph::Direction;

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    pub fn area(&self) -> f64 {
        self.width * self.height
    }
}

#[derive(Debug, Clone)]
pub struct Tile {
    pub node: TreeIndex,
    pub rect: Rect,
    pub depth: usize,
}

const MIN_TILE_AREA_PX: f64 = 4.0;

pub fn squarify(tree: &Tree, root: TreeIndex, bounds: Rect) -> Vec<Tile> {
    let mut tiles = Vec::new();
    squarify_node(tree, root, bounds, 0, &mut tiles);
    tiles
}

fn squarify_node(tree: &Tree, node: TreeIndex, rect: Rect, depth: usize, tiles: &mut Vec<Tile>) {
    let entry = match tree.node_weight(node) {
        Some(e) => e,
        None => return,
    };

    tiles.push(Tile { node, rect, depth });

    if rect.area() < MIN_TILE_AREA_PX {
        return;
    }

    let is_leaf = entry.entry_count.is_none() && !entry.is_dir;
    if is_leaf {
        return;
    }

    let mut children: Vec<_> = tree
        .neighbors_directed(node, Direction::Outgoing)
        .filter_map(|idx| {
            let child = tree.node_weight(idx)?;
            if child.size > 0 {
                Some((idx, child.size as f64))
            } else {
                None
            }
        })
        .collect();

    if children.is_empty() {
        return;
    }

    children.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    layout_children(&children, rect, tree, depth + 1, tiles);
}

fn layout_children(
    children: &[(TreeIndex, f64)],
    rect: Rect,
    tree: &Tree,
    depth: usize,
    tiles: &mut Vec<Tile>,
) {
    if children.is_empty() || rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }

    let total_size: f64 = children.iter().map(|(_, s)| s).sum();
    if total_size <= 0.0 {
        return;
    }

    let scale = rect.area() / total_size;
    let mut remaining = rect;
    let mut head = 0;

    while head < children.len() && remaining.width > 0.0 && remaining.height > 0.0 {
        let horizontal = remaining.width >= remaining.height;
        let side = if horizontal { remaining.height } else { remaining.width };

        let (row_end, row_size) = find_best_row(children, head, side * side, scale);

        let row_width = if row_size < total_size {
            row_size * scale / side
        } else {
            if horizontal { remaining.width } else { remaining.height }
        };

        let row_rect = if horizontal {
            Rect::new(remaining.x, remaining.y, row_width, remaining.height)
        } else {
            Rect::new(remaining.x, remaining.y, remaining.width, row_width)
        };

        layout_row(&children[head..row_end], row_rect, horizontal, tree, depth, tiles);

        if horizontal {
            remaining.x += row_width;
            remaining.width -= row_width;
        } else {
            remaining.y += row_width;
            remaining.height -= row_width;
        }

        head = row_end;
    }
}

fn find_best_row(children: &[(TreeIndex, f64)], start: usize, hh: f64, scale: f64) -> (usize, f64) {
    let mut sum = 0.0;
    let mut rmax = children[start].1;
    let mut worst = f64::MAX;
    let mut end = start;

    for (i, (_, size)) in children.iter().enumerate().skip(start) {
        if *size <= 0.0 {
            break;
        }

        let rmin = *size;
        let new_sum = sum + rmin;
        let ss = new_sum * new_sum * scale * scale;

        let ratio1 = hh * rmax * scale / ss;
        let ratio2 = ss / (hh * rmin * scale);
        let next_worst = ratio1.max(ratio2);

        if next_worst > worst && i > start {
            break;
        }

        sum = new_sum;
        worst = next_worst;
        end = i + 1;

        if rmin > rmax {
            rmax = rmin;
        }
    }

    (end, sum)
}

fn layout_row(
    row: &[(TreeIndex, f64)],
    rect: Rect,
    horizontal: bool,
    tree: &Tree,
    depth: usize,
    tiles: &mut Vec<Tile>,
) {
    let total: f64 = row.iter().map(|(_, s)| s).sum();
    if total <= 0.0 {
        return;
    }

    let mut pos = if horizontal { rect.y } else { rect.x };
    let length = if horizontal { rect.height } else { rect.width };

    for (i, (node, size)) in row.iter().enumerate() {
        let fraction = size / total;
        let extent = fraction * length;
        let is_last = i == row.len() - 1;

        let actual_extent = if is_last {
            (if horizontal { rect.y + rect.height } else { rect.x + rect.width }) - pos
        } else {
            extent
        };

        let child_rect = if horizontal {
            Rect::new(rect.x, pos, rect.width, actual_extent)
        } else {
            Rect::new(pos, rect.y, actual_extent, rect.height)
        };

        squarify_node(tree, *node, child_rect, depth, tiles);
        pos += extent;
    }
}

pub fn hit_test(tiles: &[Tile], x: f64, y: f64) -> Option<TreeIndex> {
    tiles
        .iter()
        .rev()
        .find(|t| {
            x >= t.rect.x
                && x < t.rect.x + t.rect.width
                && y >= t.rect.y
                && y < t.rect.y + t.rect.height
        })
        .map(|t| t.node)
}
