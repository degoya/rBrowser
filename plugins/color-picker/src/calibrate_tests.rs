use super::*;

/// The display's colours of the owner's Windows machine (diag9): `#ff00ff`, `#00ff00`.
pub(super) const MANAGED: [Rgb; 2] = [[231, 70, 251], [124, 248, 59]];
pub(super) const PAGE: Rgb = [174, 215, 237];
const FRAME: Rgb = [31, 31, 31];
/// A marker's black frame (`page/picker.js`).
const BLACK: Rgb = [0, 0, 0];

/// A view's window image: `image` pixels, the viewport of `port` CSS px drawn from the image pixel
/// `offset` at `scale` image px per CSS px, the markers at `boxes` (their frames' corners) in
/// `colours` but for those `hidden` (covered, not drawn), a `bar` of a colour along the viewport's
/// bottom (30 CSS px) under them, the frame's colour around it. Each image pixel is the mean of
/// 4 x 4 points of the CSS area it covers, so a scaled marker's edges blend as a compositor's do.
pub(super) struct Shot {
    pub(super) image: (u32, u32),
    pub(super) port: (f64, f64),
    pub(super) offset: (f64, f64),
    pub(super) scale: f64,
    pub(super) boxes: Marker,
    pub(super) colours: [Rgb; 2],
    pub(super) hidden: Vec<usize>,
    pub(super) bar: Option<Rgb>,
}

impl Shot {
    fn css(&self, x: f64, y: f64) -> Rgb {
        if x < 0.0 || y < 0.0 || x >= self.port.0 || y >= self.port.1 {
            return FRAME;
        }
        for (index, (left, top)) in self.boxes.iter().enumerate() {
            if self.hidden.contains(&index) {
                continue;
            }
            let (dx, dy) = (x - left, y - top);
            let outer = (
                MARKER_SIZE.0 + 2.0 * MARKER_FRAME,
                MARKER_SIZE.1 + 2.0 * MARKER_FRAME,
            );
            if !((0.0..outer.0).contains(&dx) && (0.0..outer.1).contains(&dy)) {
                continue;
            }
            let (cx, cy) = (dx - MARKER_FRAME, dy - MARKER_FRAME);
            if (0.0..MARKER_SIZE.0).contains(&cx) && (0.0..MARKER_SIZE.1).contains(&cy) {
                return self.colours[usize::from(cx >= MARKER_SIZE.0 / 2.0)];
            }
            return BLACK;
        }
        match self.bar {
            Some(bar) if y >= self.port.1 - 30.0 => bar,
            _ => PAGE,
        }
    }

    fn pixel(&self, ix: u32, iy: u32) -> Rgb {
        // Inside one area (the corners agree, the areas being far larger than a pixel): its colour.
        let at = |dx: f64, dy: f64| {
            self.css(
                (f64::from(ix) + dx - self.offset.0) / self.scale,
                (f64::from(iy) + dy - self.offset.1) / self.scale,
            )
        };
        let corner = at(0.01, 0.01);
        if [at(0.99, 0.01), at(0.01, 0.99), at(0.99, 0.99)]
            .iter()
            .all(|other| *other == corner)
        {
            return corner;
        }
        let mut sum = [0.0_f64; 3];
        for sy in 0..4 {
            for sx in 0..4 {
                let x = (f64::from(ix) + (f64::from(sx) + 0.5) / 4.0 - self.offset.0) / self.scale;
                let y = (f64::from(iy) + (f64::from(sy) + 0.5) / 4.0 - self.offset.1) / self.scale;
                for (total, channel) in sum.iter_mut().zip(self.css(x, y)) {
                    *total += f64::from(channel);
                }
            }
        }
        sum.map(|total| (total / 16.0).round() as u8)
    }

    pub(super) fn bands(&self) -> Vec<Band> {
        let mut finder = Finder::default();
        for y in 0..self.image.1 {
            let row: Vec<Rgb> = (0..self.image.0).map(|x| self.pixel(x, y)).collect();
            finder.row(y, &row);
        }
        finder.markers()
    }

    pub(super) fn calibrate(&self, dpr: f64) -> Result<(Map, bool), String> {
        calibrate(
            &self.bands(),
            &self.boxes,
            (self.port.0, self.port.1, dpr),
            self.image,
        )
    }
}

/// The markers' frames where page/picker.js draws them: the viewport's four corners, top left,
/// top right, bottom left, bottom right.
pub(super) fn corners(width: f64, height: f64) -> Marker {
    let (right, bottom) = (
        width - MARKER_SIZE.0 - 2.0 * MARKER_FRAME,
        height - MARKER_SIZE.1 - 2.0 * MARKER_FRAME,
    );
    vec![(0.0, 0.0), (right, 0.0), (0.0, bottom), (right, bottom)]
}

pub(super) fn shot(image: (u32, u32), port: (f64, f64), offset: (f64, f64), scale: f64) -> Shot {
    Shot {
        image,
        port,
        offset,
        scale,
        boxes: corners(port.0, port.1),
        colours: MANAGED,
        hidden: Vec::new(),
        bar: None,
    }
}

#[test]
fn the_display_s_colours_of_the_markers_are_found() {
    // Red before the fix (src/shot.rs `shows_marker`): RB-080-64 wanted the marker's own values
    // within 40 and every read of the owner's views went to a clip, which flickers.
    assert_eq!(class(MANAGED[0]), Class::Magenta);
    assert_eq!(class(MANAGED[1]), Class::Green);
    assert_eq!(class(MARKER[0]), Class::Magenta);
    assert_eq!(class(MARKER[1]), Class::Green);
    for other in [
        PAGE,
        FRAME,
        [255, 255, 255],
        [16, 23, 42],
        [81, 159, 188],
        [145, 23, 42],
    ] {
        assert_eq!(class(other), Class::Other, "{other:?}");
    }
}

#[test]
fn the_sizes_of_diag9_map_the_viewport_at_the_view_s_scale() {
    // Every window image of diag9 (2026-11-15, Windows, build f68e917): `w x h px for W x H CSS
    // px`, the scale its cell shows the viewport at.
    for (image, port, dpr, scale) in [
        ((635, 365), (1024.0, 588.0), 1.0, 635.0 / 1024.0),
        ((768, 453), (1024.0, 604.0), 1.0, 0.75),
        ((2049, 1098), (2732.0, 1464.0), 1.0, 2049.0 / 2732.0),
        ((2049, 737), (2732.0, 982.0), 1.0, 2049.0 / 2732.0),
        ((1920, 1136), (2560.0, 1514.0), 1.0, 0.75),
        ((1536, 906), (1024.0, 604.0), 1.5, 1.5),
        ((3000, 1001), (2000.0, 667.0), 1.5, 1.5),
    ] {
        let shot = shot(image, port, (0.0, 0.0), scale);
        let (map, exact) = shot.calibrate(dpr).expect("calibrated");
        assert_eq!((map.left, map.top), (0, 0), "{image:?}");
        assert!((map.scale - scale).abs() < 1e-9, "{image:?}: {}", map.scale);
        assert_eq!((map.width, map.height), image);
        // The display's colours: the page's own are not what the image shows.
        assert!(!exact);
    }
}

#[test]
fn an_image_of_the_viewport_at_its_dpr_is_mapped_by_its_size_whatever_it_shows() {
    // RB-080-77 read diag9's `1536 x 906 px for 1024 x 604 CSS px` at DPR 1.5 (first marker found,
    // the second not) as a page drawn at another scale and let the markers measure it. diag11
    // (RB-090-19) shows the second marker simply missing and false boxes measured instead: an
    // image of the viewport at its DPR is the viewport at its DPR, whatever boxes it shows.
    let shot = shot((1536, 906), (1024.0, 604.0), (0.0, 0.0), 1.0);
    let (map, _) = shot.calibrate(1.5).expect("calibrated");
    assert_eq!((map.left, map.top, map.scale), (0, 0, 1.5));
}

#[test]
fn a_viewport_away_from_the_image_s_corner_is_mapped_with_its_offset() {
    let shot = shot((700, 400), (1200.0, 700.0), (40.0, 12.0), 0.5);
    let (map, _) = shot.calibrate(1.0).expect("calibrated");
    assert_eq!((map.left, map.top), (40, 12));
    assert!((map.scale - 0.5).abs() < 0.005, "{}", map.scale);
}

#[test]
fn exact_colours_say_the_image_is_the_page_s() {
    let mut exact = shot((800, 600), (400.0, 300.0), (0.0, 0.0), 2.0);
    exact.colours = MARKER;
    assert_eq!(
        exact.calibrate(2.0).map(|(map, exact)| (map.scale, exact)),
        Ok((2.0, true))
    );
}

#[test]
fn an_image_without_both_markers_is_not_mapped() {
    // diag9's other reads: the page's colour where the markers were - the window's image of the
    // frame before they were drawn. The page script asks again.
    let mut none = shot((635, 365), (1024.0, 588.0), (0.0, 0.0), 635.0 / 1024.0);
    none.hidden = vec![0, 1, 2, 3];
    let missing = none.calibrate(1.0).expect_err("no markers");
    assert!(missing.contains("shows 0 of the 4 markers"), "{missing}");
    let mut one = shot((635, 365), (1024.0, 588.0), (0.0, 0.0), 635.0 / 1024.0);
    one.hidden = vec![1, 2, 3];
    assert_eq!(one.bands().len(), 1);
    assert!(one.calibrate(1.0).is_err());
    let mut wrong = shot((635, 365), (1024.0, 588.0), (0.0, 0.0), 635.0 / 1024.0);
    // The page script's boxes do not match what the image shows: no scale maps both.
    let drawn = wrong.bands();
    wrong.boxes = vec![(0.0, 0.0), (300.0, 20.0)];
    assert!(calibrate(&drawn, &wrong.boxes, (1024.0, 588.0, 1.0), (635, 365)).is_err());
}

#[test]
fn a_page_s_own_magenta_and_green_is_no_marker() {
    // A magenta area beside a green one of another shape: one row, or far from 2:1.
    let mut finder = Finder::default();
    let row: Vec<Rgb> = (0..60)
        .map(|x| if x < 30 { MARKER[0] } else { MARKER[1] })
        .collect();
    for y in 0..60 {
        finder.row(y, &row);
    }
    assert!(finder.markers().is_empty());
    let mut thin = Finder::default();
    thin.row(0, &row[20..40]);
    assert!(thin.markers().is_empty());
}
