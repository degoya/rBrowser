//! Redaction rendered into the pixels: a mosaic, a smoothed mosaic or a black bar. The mosaic's
//! blocks start at the region's top left and each becomes the rounded average of its pixels, so
//! the result holds one colour per block and nothing finer; the smoothed mode is computed from
//! the mosaic alone (never from the original), so it hides exactly as much. The panel's preview
//! (`web/src/panel/draw.ts`) does the same arithmetic.

use tiny_skia::Pixmap;

/// A region in whole pixels, clipped to the image: `left..right`, `top..bottom`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}

impl Region {
    /// The pixels `x`, `y`, `w`, `h` touches (outward to whole pixels), inside a `width` x
    /// `height` image; none when nothing of it is inside.
    #[must_use]
    pub fn clip(x: f32, y: f32, w: f32, h: f32, width: u32, height: u32) -> Option<Self> {
        let edge = |value: f32, limit: u32| value.clamp(0.0, limit as f32);
        let left = edge(x.floor(), width) as u32;
        let top = edge(y.floor(), height) as u32;
        let right = edge((x + w).ceil(), width) as u32;
        let bottom = edge((y + h).ceil(), height) as u32;
        (right > left && bottom > top).then_some(Self {
            left,
            top,
            right,
            bottom,
        })
    }
}

fn index(pixmap: &Pixmap, x: u32, y: u32) -> usize {
    ((y * pixmap.width() + x) * 4) as usize
}

/// Every block of `block` pixels in `region` becomes the rounded average of its pixels.
pub fn pixelate(pixmap: &mut Pixmap, region: Region, block: u32) {
    let block = block.max(1);
    let mut top = region.top;
    while top < region.bottom {
        let bottom = (top + block).min(region.bottom);
        let mut left = region.left;
        while left < region.right {
            let right = (left + block).min(region.right);
            let mut sum = [0u64; 4];
            for y in top..bottom {
                for x in left..right {
                    let i = index(pixmap, x, y);
                    for (channel, total) in sum.iter_mut().enumerate() {
                        *total += u64::from(pixmap.data()[i + channel]);
                    }
                }
            }
            let count = u64::from((right - left) * (bottom - top));
            let average = sum.map(|total| u8::try_from((total + count / 2) / count).unwrap_or(0));
            for y in top..bottom {
                for x in left..right {
                    let i = index(pixmap, x, y);
                    pixmap.data_mut()[i..i + 4].copy_from_slice(&average);
                }
            }
            left = right;
        }
        top = bottom;
    }
}

/// One pass of a box blur of `radius` along rows (`horizontal`) or columns, inside `region`
/// only, the edge pixels repeated.
fn box_pass(pixmap: &mut Pixmap, region: Region, radius: u32, horizontal: bool) {
    let (outer, inner) = if horizontal {
        (region.top..region.bottom, region.left..region.right)
    } else {
        (region.left..region.right, region.top..region.bottom)
    };
    let (start, end) = (inner.start, inner.end);
    let mut line = Vec::with_capacity((end - start) as usize);
    for o in outer {
        let at = |i: u32| if horizontal { (i, o) } else { (o, i) };
        line.clear();
        for i in start..end {
            let (x, y) = at(i);
            let p = index(pixmap, x, y);
            line.push([
                pixmap.data()[p],
                pixmap.data()[p + 1],
                pixmap.data()[p + 2],
                pixmap.data()[p + 3],
            ]);
        }
        let last = line.len() as i64 - 1;
        let span = i64::from(radius);
        for i in start..end {
            let centre = i64::from(i - start);
            let mut sum = [0u64; 4];
            for offset in -span..=span {
                let pixel = line[(centre + offset).clamp(0, last) as usize];
                for channel in 0..4 {
                    sum[channel] += u64::from(pixel[channel]);
                }
            }
            let count = (2 * span + 1) as u64;
            let (x, y) = at(i);
            let p = index(pixmap, x, y);
            for channel in 0..4 {
                pixmap.data_mut()[p + channel] =
                    u8::try_from((sum[channel] + count / 2) / count).unwrap_or(0);
            }
        }
    }
}

/// The mosaic of `block`, then smoothed with a box of half a block, rows then columns.
pub fn blur(pixmap: &mut Pixmap, region: Region, block: u32) {
    pixelate(pixmap, region, block);
    let radius = (block / 2).max(1);
    box_pass(pixmap, region, radius, true);
    box_pass(pixmap, region, radius, false);
}

/// Opaque black.
pub fn black(pixmap: &mut Pixmap, region: Region) {
    for y in region.top..region.bottom {
        for x in region.left..region.right {
            let i = index(pixmap, x, y);
            pixmap.data_mut()[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
    }
}

#[cfg(test)]
#[path = "redact_tests.rs"]
mod tests;
