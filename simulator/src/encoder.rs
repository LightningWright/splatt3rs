pub fn diff_rect(a: &[u8], b: &[u8], w: usize, h: usize) -> Option<(usize, usize, usize, usize)> {
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut changed = false;

    for y in 0..h {
        let row = y * w;
        for x in 0..w {
            let i = row + x;
            if a[i] != b[i] {
                changed = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    if changed {
        Some((min_x, min_y, max_x + 1, max_y + 1))
    } else {
        None
    }
}

pub fn crop(pixels: &[u8], w: usize, x0: usize, y0: usize, x1: usize, y1: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity((x1 - x0) * (y1 - y0));
    for y in y0..y1 {
        out.extend_from_slice(&pixels[y * w + x0..y * w + x1]);
    }
    out
}

pub fn rgb_to_indexed(rgb: &[u8], palette: &[u8]) -> Vec<u8> {
    rgb.chunks_exact(3)
        .map(|px| {
            for i in 0..(palette.len() / 3) {
                let p = &palette[i * 3..i * 3 + 3];
                if px == p {
                    return i as u8;
                }
            }
            panic!("Pixel {:?} not found in palette", px);
        })
        .collect()
}
