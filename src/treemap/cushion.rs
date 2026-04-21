use super::layout::{Rect, Tile};
use super::palette::PALETTE_BRIGHTNESS;

const DEFAULT_HEIGHT: f64 = 0.38;
const DEFAULT_SCALE_FACTOR: f64 = 0.91;
const DEFAULT_AMBIENT_LIGHT: f64 = 0.13;
const DEFAULT_BRIGHTNESS: f64 = 0.88;
const LIGHT_SOURCE_X: f64 = -1.0;
const LIGHT_SOURCE_Y: f64 = -1.0;
const LIGHT_SOURCE_Z: f64 = 10.0;

#[derive(Debug, Clone, Copy)]
pub struct CushionOptions {
    pub height: f64,
    pub scale_factor: f64,
    pub ambient_light: f64,
    pub brightness: f64,
    pub light_x: f64,
    pub light_y: f64,
    pub light_z: f64,
}

impl Default for CushionOptions {
    fn default() -> Self {
        Self {
            height: DEFAULT_HEIGHT,
            scale_factor: DEFAULT_SCALE_FACTOR,
            ambient_light: DEFAULT_AMBIENT_LIGHT,
            brightness: DEFAULT_BRIGHTNESS,
            light_x: LIGHT_SOURCE_X,
            light_y: LIGHT_SOURCE_Y,
            light_z: LIGHT_SOURCE_Z,
        }
    }
}

impl CushionOptions {
    fn normalized_light(&self) -> (f64, f64, f64) {
        let len = (self.light_x.powi(2) + self.light_y.powi(2) + self.light_z.powi(2)).sqrt();
        (self.light_x / len, self.light_y / len, self.light_z / len)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Surface {
    s0: f64,
    s1: f64,
    s2: f64,
    s3: f64,
}

impl Surface {
    fn add_ridge(&mut self, rect: &Rect, h: f64) {
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }

        let h4 = 4.0 * h;

        let wf = h4 / rect.width;
        self.s2 += wf * (rect.x + rect.x + rect.width);
        self.s0 -= wf;

        let hf = h4 / rect.height;
        self.s3 += hf * (rect.y + rect.y + rect.height);
        self.s1 -= hf;
    }
}

pub fn render_cushions<F>(
    tiles: &[Tile],
    color_of: F,
    width: u32,
    height: u32,
    options: &CushionOptions,
) -> Vec<u8>
where
    F: Fn(usize) -> [u8; 3] + Sync,
{
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let (lx, ly, lz) = options.normalized_light();

    let leaf_tiles: Vec<_> = tiles
        .iter()
        .enumerate()
        .filter(|(_, t)| t.rect.width >= 1.0 && t.rect.height >= 1.0)
        .collect();

    for (tile_idx, tile) in leaf_tiles {
        let color = color_of(tile_idx);
        let mut surface = Surface::default();

        compute_surface_for_tile(tiles, tile_idx, options, &mut surface);

        draw_cushion_tile(
            &mut pixels,
            width,
            height,
            tile,
            &surface,
            color,
            options,
            lx,
            ly,
            lz,
        );
    }

    pixels
}

fn compute_surface_for_tile(tiles: &[Tile], tile_idx: usize, options: &CushionOptions, surface: &mut Surface) {
    let tile = &tiles[tile_idx];
    let mut h = options.height;

    for ancestor in tiles.iter().take(tile_idx) {
        if ancestor.depth < tile.depth && contains(&ancestor.rect, &tile.rect) {
            surface.add_ridge(&ancestor.rect, h);
            h *= options.scale_factor;
        }
    }

    surface.add_ridge(&tile.rect, h);
}

fn contains(outer: &Rect, inner: &Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width + 0.01
        && inner.y + inner.height <= outer.y + outer.height + 0.01
}

fn draw_cushion_tile(
    pixels: &mut [u8],
    img_width: u32,
    img_height: u32,
    tile: &Tile,
    surface: &Surface,
    color: [u8; 3],
    options: &CushionOptions,
    lx: f64,
    ly: f64,
    lz: f64,
) {
    let ia = options.ambient_light;
    let is = 1.0 - ia;

    let col_r = color[0] as f64;
    let col_g = color[1] as f64;
    let col_b = color[2] as f64;

    let x_start = tile.rect.x.floor().max(0.0) as u32;
    let y_start = tile.rect.y.floor().max(0.0) as u32;
    let x_end = ((tile.rect.x + tile.rect.width).ceil() as u32).min(img_width);
    let y_end = ((tile.rect.y + tile.rect.height).ceil() as u32).min(img_height);

    for iy in y_start..y_end {
        for ix in x_start..x_end {
            let nx = -(2.0 * surface.s0 * (ix as f64 + 0.5) + surface.s2);
            let ny = -(2.0 * surface.s1 * (iy as f64 + 0.5) + surface.s3);

            let cosa = (nx * lx + ny * ly + lz) / (nx * nx + ny * ny + 1.0).sqrt();
            let cosa = cosa.min(1.0);

            let mut pixel = is * cosa;
            pixel = pixel.max(0.0);
            pixel += ia;

            pixel *= options.brightness / PALETTE_BRIGHTNESS;

            let mut red = (col_r * pixel) as i32;
            let mut green = (col_g * pixel) as i32;
            let mut blue = (col_b * pixel) as i32;

            normalize_color(&mut red, &mut green, &mut blue);

            let idx = ((iy * img_width + ix) * 4) as usize;
            pixels[idx] = red as u8;
            pixels[idx + 1] = green as u8;
            pixels[idx + 2] = blue as u8;
            pixels[idx + 3] = 255;
        }
    }
}

fn normalize_color(red: &mut i32, green: &mut i32, blue: &mut i32) {
    if *red > 255 {
        distribute_first(red, green, blue);
    } else if *green > 255 {
        distribute_first(green, red, blue);
    } else if *blue > 255 {
        distribute_first(blue, red, green);
    }

    *red = (*red).clamp(0, 255);
    *green = (*green).clamp(0, 255);
    *blue = (*blue).clamp(0, 255);
}

fn distribute_first(first: &mut i32, second: &mut i32, third: &mut i32) {
    let h = (*first - 255) / 2;
    *first = 255;
    *second += h;
    *third += h;

    if *second > 255 {
        let j = *second - 255;
        *second = 255;
        *third += j;
    } else if *third > 255 {
        let j = *third - 255;
        *third = 255;
        *second += j;
    }
}
