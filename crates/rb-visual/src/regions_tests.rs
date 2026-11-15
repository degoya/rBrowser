use super::*;

fn mask(width: u32, height: u32, set: &[(u32, u32)]) -> Vec<bool> {
    let mut mask = vec![false; (width * height) as usize];
    for (x, y) in set {
        mask[(y * width + x) as usize] = true;
    }
    mask
}

#[test]
fn no_differing_pixel_is_no_region() {
    assert_eq!(regions(&mask(40, 40, &[]), 40, 40), (Vec::new(), false));
    assert_eq!(regions(&[], 0, 0), (Vec::new(), false));
}

#[test]
fn close_pixels_are_one_box_around_them() {
    let (found, truncated) = regions(&mask(64, 64, &[(3, 4), (20, 9), (17, 30)]), 64, 64);
    assert!(!truncated);
    assert_eq!(
        found,
        [Region {
            x: 3,
            y: 4,
            width: 18,
            height: 27,
            pixels: 3,
        }]
    );
}

#[test]
fn far_apart_pixels_are_two_boxes_the_larger_first() {
    let (found, _) = regions(
        &mask(200, 100, &[(2, 2), (150, 80), (151, 80), (150, 81)]),
        200,
        100,
    );
    assert_eq!(found.len(), 2);
    assert_eq!(
        found[0],
        Region {
            x: 150,
            y: 80,
            width: 2,
            height: 2,
            pixels: 3,
        }
    );
    assert_eq!((found[1].x, found[1].y, found[1].pixels), (2, 2, 1));
}

#[test]
fn more_areas_than_the_limit_are_cut_and_flagged() {
    // One pixel in every other tile of a row: no two tiles touch.
    let width = TILE * 2 * (MAX_REGIONS as u32 + 4);
    let set: Vec<(u32, u32)> = (0..MAX_REGIONS as u32 + 4)
        .map(|n| (n * TILE * 2, 0))
        .collect();
    let (found, truncated) = regions(&mask(width, 1, &set), width, 1);
    assert_eq!(found.len(), MAX_REGIONS);
    assert!(truncated);
}
