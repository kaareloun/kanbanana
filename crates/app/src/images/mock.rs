//! Generated images for `--mock`: a light admin page screenshot and avatar portraits.
//! Image content, so the colors are fixed and do not follow the theme.

use image::{ImageFormat, Rgba, RgbaImage};

const PAPER: [u8; 3] = [0xff, 0xfa, 0xf3];
const BAR: [u8; 3] = [0xf2, 0xe9, 0xe1];
const LINE: [u8; 3] = [0xdf, 0xda, 0xd9];
const INK: [u8; 3] = [0x57, 0x52, 0x79];
const SOFT: [u8; 3] = [0x97, 0x93, 0xa5];
const HOT: [u8; 3] = [0xb4, 0x63, 0x7a];
const HOT_BG: [u8; 3] = [0xf5, 0xe6, 0xe4];
const PAID: [u8; 3] = [0x28, 0x69, 0x83];
const PAID_BG: [u8; 3] = [0xde, 0xec, 0xe9];
const DOTS: [[u8; 3]; 3] = [[0xd7, 0x82, 0x7e], [0xea, 0x9d, 0x34], [0x56, 0x94, 0x9f]];

fn px(c: [u8; 3]) -> Rgba<u8> {
    Rgba([c[0], c[1], c[2], 255])
}

fn rect(img: &mut RgbaImage, x: i64, y: i64, w: i64, h: i64, c: [u8; 3]) {
    let (iw, ih) = (img.width() as i64, img.height() as i64);
    for yy in y.max(0)..(y + h).min(ih) {
        for xx in x.max(0)..(x + w).min(iw) {
            img.put_pixel(xx as u32, yy as u32, px(c));
        }
    }
}

fn outline(img: &mut RgbaImage, x: i64, y: i64, w: i64, h: i64, c: [u8; 3]) {
    rect(img, x, y, w, 1, c);
    rect(img, x, y + h - 1, w, 1, c);
    rect(img, x, y, 1, h, c);
    rect(img, x + w - 1, y, 1, h, c);
}

fn dot(img: &mut RgbaImage, cx: i64, cy: i64, r: i64, c: [u8; 3]) {
    for yy in -r..=r {
        for xx in -r..=r {
            if xx * xx + yy * yy <= r * r {
                rect(img, cx + xx, cy + yy, 1, 1, c);
            }
        }
    }
}

/// Encodes a light "screenshot" of an order page as PNG: browser bar, a title,
/// a status badge and log rows with the last one marked. Text is drawn as bars.
pub fn mock_png(width: u32, height: u32) -> Vec<u8> {
    let (w, h) = (width.max(40) as i64, height.max(40) as i64);
    let mut img = RgbaImage::from_pixel(w as u32, h as u32, px(PAPER));
    let bar_h = 30.min(h / 4);
    rect(&mut img, 0, 0, w, bar_h, BAR);
    for (i, c) in DOTS.iter().enumerate() {
        dot(&mut img, 16 + i as i64 * 14, bar_h / 2, 4, *c);
    }
    rect(&mut img, 62, bar_h / 2 - 3, (w / 3).min(220), 6, SOFT);

    let head_y = bar_h + 14;
    rect(&mut img, 12, head_y, 96.min(w / 3), 10, INK);
    let badge_w = 40;
    rect(&mut img, w - 12 - badge_w, head_y - 4, badge_w, 18, PAID_BG);
    rect(&mut img, w - 12 - badge_w + 10, head_y + 2, badge_w - 20, 6, PAID);

    let row_h = 30;
    let mut y = head_y + 24;
    let rows = ((h - y - 10) / row_h).max(1);
    for i in 0..rows {
        let hot = i == rows - 1;
        let (x0, rw) = (12, w - 24);
        if hot {
            rect(&mut img, x0, y, rw, row_h, HOT_BG);
            outline(&mut img, x0, y, rw, row_h, HOT);
        } else {
            rect(&mut img, x0, y + row_h - 1, rw, 1, LINE);
        }
        let ty = y + row_h / 2 - 3;
        rect(&mut img, x0 + 12, ty, 56, 6, INK);
        rect(&mut img, x0 + 96, ty, (rw / 3).min(160), 6, INK);
        rect(&mut img, x0 + rw - 48, ty, 36, 6, if hot { HOT } else { SOFT });
        y += row_h;
    }

    encode(&img)
}

/// Backgrounds for mock portraits. Muted studio colors.
const PORTRAIT_BG: [[u8; 3]; 6] = [
    [0x6d, 0x5a, 0x8c],
    [0x3f, 0x6e, 0x7a],
    [0x9a, 0x6a, 0x3c],
    [0x8a, 0x4f, 0x5e],
    [0x4d, 0x6b, 0x4f],
    [0x5a, 0x63, 0x8a],
];

fn lighten(c: [u8; 3], amount: f32) -> [u8; 3] {
    c.map(|v| (v as f32 + (255.0 - v as f32) * amount).round() as u8)
}

fn encode(img: &RgbaImage) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    // Writing to memory fails only on a broken encoder. An empty result shows as a failed image.
    if img.write_to(&mut out, ImageFormat::Png).is_err() {
        return Vec::new();
    }
    out.into_inner()
}

/// A 96x96 portrait: colored background, lighter head and shoulders. The URL picks the color.
pub fn mock_portrait(url: &str) -> Vec<u8> {
    let hash = url.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    let bg = PORTRAIT_BG[hash as usize % PORTRAIT_BG.len()];
    let fg = lighten(bg, 0.45);
    let mut img = RgbaImage::from_pixel(96, 96, px(bg));
    // Shoulders: the top half of an ellipse that leaves the bottom edge.
    let (cx, cy, rx, ry) = (48.0f32, 104.0f32, 36.0f32, 40.0f32);
    for y in 60..96 {
        for x in 0..96 {
            let (dx, dy) = ((x as f32 - cx) / rx, (y as f32 - cy) / ry);
            if dx * dx + dy * dy <= 1.0 {
                img.put_pixel(x, y, px(fg));
            }
        }
    }
    dot(&mut img, 48, 40, 19, fg);
    encode(&img)
}

/// Bytes for the mock upload and avatar URLs. The size follows the design for each image.
pub fn mock_bytes(url: &str) -> Vec<u8> {
    if url.contains("/mock/avatar-") {
        mock_portrait(url)
    } else if url.contains("4502") {
        mock_png(300, 128)
    } else {
        mock_png(564, 132)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn mock_png_decodes() {
        let bytes = super::mock_png(300, 128);
        let img = image::load_from_memory(&bytes).unwrap();
        assert_eq!((img.width(), img.height()), (300, 128));
    }

    #[test]
    fn mock_portrait_decodes_for_avatar_urls() {
        let bytes = super::mock_bytes("https://uploads.linear.app/mock/avatar-ka.png");
        let img = image::load_from_memory(&bytes).unwrap();
        assert_eq!((img.width(), img.height()), (96, 96));
    }
}
