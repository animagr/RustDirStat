pub const PALETTE_BRIGHTNESS: f64 = 0.6;

pub const DEFAULT_CUSHION_COLORS: [[u8; 3]; 18] = [
    [0, 0, 255],     // Blue
    [255, 0, 0],     // Red
    [0, 255, 0],     // Green
    [255, 255, 0],   // Yellow
    [0, 255, 255],   // Cyan
    [255, 0, 255],   // Magenta
    [255, 170, 0],   // Orange
    [0, 85, 255],    // Dodger Blue
    [255, 0, 85],    // Hot Pink
    [85, 255, 0],    // Lime Green
    [170, 0, 255],   // Violet
    [0, 255, 85],    // Spring Green
    [255, 0, 170],   // Deep Pink
    [0, 170, 255],   // Sky Blue
    [255, 85, 0],    // Orange Red
    [0, 255, 170],   // Aquamarine
    [85, 0, 255],    // Indigo
    [255, 255, 255], // White
];

pub const FALLBACK_COLOR: [u8; 3] = [128, 128, 128];

pub fn make_bright_color(color: [u8; 3], brightness: f64) -> [u8; 3] {
    let r = color[0] as f64 / 255.0;
    let g = color[1] as f64 / 255.0;
    let b = color[2] as f64 / 255.0;

    let sum = r + g + b;
    if sum < 0.001 {
        return [0, 0, 0];
    }

    let f = 3.0 * brightness / sum;
    let mut red = (r * f * 255.0) as i32;
    let mut green = (g * f * 255.0) as i32;
    let mut blue = (b * f * 255.0) as i32;

    normalize_color(&mut red, &mut green, &mut blue);

    [red as u8, green as u8, blue as u8]
}

pub fn normalize_color(red: &mut i32, green: &mut i32, blue: &mut i32) {
    if *red > 255 {
        distribute_first(red, green, blue);
    } else if *green > 255 {
        distribute_first(green, red, blue);
    } else if *blue > 255 {
        distribute_first(blue, red, green);
    }
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

pub fn get_palette() -> Vec<[u8; 3]> {
    DEFAULT_CUSHION_COLORS
        .iter()
        .map(|&c| make_bright_color(c, PALETTE_BRIGHTNESS))
        .collect()
}
