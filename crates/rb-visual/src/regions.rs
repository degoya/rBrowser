//! The bounding boxes of a diff: the differing pixels gathered in tiles of [`TILE`] pixels, tiles
//! that touch (also at a corner) joined into one area, each area's box the smallest rectangle
//! around its pixels. A change a few pixels apart stays one box; two buttons far apart are two.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// The side of a tile in pixels: differing pixels closer than about this are one area.
pub const TILE: u32 = 16;
/// The most areas a diff names; the smallest beyond it are left out.
pub const MAX_REGIONS: usize = 32;

/// One differing area of a diff, in pixels of the image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    /// Left.
    pub x: u32,
    /// Top.
    pub y: u32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// The differing pixels inside.
    pub pixels: u64,
}

/// What a tile holds: its differing pixels and their extent.
#[derive(Clone, Copy)]
struct Tile {
    pixels: u64,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

/// The areas of `mask` (one flag per pixel, rows top to bottom, `width` x `height`), largest
/// first, at most [`MAX_REGIONS`]; and whether some were left out.
pub fn regions(mask: &[bool], width: u32, height: u32) -> (Vec<Region>, bool) {
    if width == 0 || height == 0 {
        return (Vec::new(), false);
    }
    let (across, down) = (width.div_ceil(TILE), height.div_ceil(TILE));
    let mut tiles: Vec<Option<Tile>> = vec![None; across as usize * down as usize];
    for (index, _) in mask.iter().enumerate().filter(|(_, differs)| **differs) {
        let (Ok(x), Ok(y)) = (
            u32::try_from(index % width as usize),
            u32::try_from(index / width as usize),
        ) else {
            continue;
        };
        let at = (y / TILE) as usize * across as usize + (x / TILE) as usize;
        let Some(slot) = tiles.get_mut(at) else {
            continue;
        };
        let tile = slot.get_or_insert(Tile {
            pixels: 0,
            left: x,
            top: y,
            right: x,
            bottom: y,
        });
        tile.pixels += 1;
        tile.left = tile.left.min(x);
        tile.top = tile.top.min(y);
        tile.right = tile.right.max(x);
        tile.bottom = tile.bottom.max(y);
    }
    let mut seen = vec![false; tiles.len()];
    let mut found = Vec::new();
    for start in 0..tiles.len() {
        if seen[start] || tiles[start].is_none() {
            continue;
        }
        seen[start] = true;
        let mut area: Option<Tile> = None;
        let mut queue = VecDeque::from([start]);
        while let Some(at) = queue.pop_front() {
            let Some(tile) = tiles[at] else {
                continue;
            };
            area = Some(match area {
                None => tile,
                Some(sum) => Tile {
                    pixels: sum.pixels + tile.pixels,
                    left: sum.left.min(tile.left),
                    top: sum.top.min(tile.top),
                    right: sum.right.max(tile.right),
                    bottom: sum.bottom.max(tile.bottom),
                },
            });
            let (tx, ty) = (at % across as usize, at / across as usize);
            for ny in ty.saturating_sub(1)..=(ty + 1).min(down as usize - 1) {
                for nx in tx.saturating_sub(1)..=(tx + 1).min(across as usize - 1) {
                    let next = ny * across as usize + nx;
                    if !seen[next] && tiles[next].is_some() {
                        seen[next] = true;
                        queue.push_back(next);
                    }
                }
            }
        }
        if let Some(area) = area {
            found.push(Region {
                x: area.left,
                y: area.top,
                width: area.right - area.left + 1,
                height: area.bottom - area.top + 1,
                pixels: area.pixels,
            });
        }
    }
    found.sort_by(|a, b| {
        b.pixels
            .cmp(&a.pixels)
            .then(a.y.cmp(&b.y))
            .then(a.x.cmp(&b.x))
    });
    let truncated = found.len() > MAX_REGIONS;
    found.truncate(MAX_REGIONS);
    (found, truncated)
}

#[cfg(test)]
#[path = "regions_tests.rs"]
mod tests;
