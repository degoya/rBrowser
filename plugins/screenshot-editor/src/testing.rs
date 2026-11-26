//! What the native tests share: images made in memory, a PNG's chunk list, one annotation of
//! each tool and a host with the plugin's grants.

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{
    Capability, Failure, Finding, HttpRequest, HttpResponse, PluginHost, ViewId, ViewInfo,
};
use serde_json::json;

/// A well-mixed hash of a pixel's place: noise no PNG filter or deflate makes smaller.
pub fn noise(x: u32, y: u32) -> [u8; 4] {
    let mut v = x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B);
    v ^= v >> 16;
    v = v.wrapping_mul(0x7FEB_352D);
    v ^= v >> 15;
    v = v.wrapping_mul(0x846C_A68B);
    v ^= v >> 16;
    [(v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, 255]
}

/// A PNG of `width` x `height` RGBA pixels, each `pixel(x, y)`.
pub fn png(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            rgba.extend_from_slice(&pixel(x, y));
        }
    }
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(&rgba, width, height, ExtendedColorType::Rgba8)
        .expect("encode");
    out
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// A PNG's signature and header (`IHDR`, 8-bit RGBA) for a `width` x `height` image, no pixels:
/// what a size check reads, at any size.
pub fn header_only(width: u32, height: u32) -> Vec<u8> {
    let mut typed = b"IHDR".to_vec();
    typed.extend_from_slice(&width.to_be_bytes());
    typed.extend_from_slice(&height.to_be_bytes());
    typed.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    out.extend_from_slice(&13u32.to_be_bytes());
    out.extend_from_slice(&typed);
    out.extend_from_slice(&crc32(&typed).to_be_bytes());
    out
}

/// `png` with a `tEXt` chunk (`keyword`, `text`) after its header, as a screenshot tool or a
/// browser may write one.
pub fn with_text(png: &[u8], keyword: &str, text: &str) -> Vec<u8> {
    // Signature (8) and IHDR (4 length, 4 type, 13 data, 4 CRC).
    let split = 8 + 25;
    let mut data = keyword.as_bytes().to_vec();
    data.push(0);
    data.extend_from_slice(text.as_bytes());
    let mut chunk = (data.len() as u32).to_be_bytes().to_vec();
    let mut typed = b"tEXt".to_vec();
    typed.extend_from_slice(&data);
    chunk.extend_from_slice(&typed);
    chunk.extend_from_slice(&crc32(&typed).to_be_bytes());
    let mut out = png[..split].to_vec();
    out.extend_from_slice(&chunk);
    out.extend_from_slice(&png[split..]);
    out
}

/// The chunk types of a PNG, in order.
pub fn chunk_types(png: &[u8]) -> Vec<String> {
    let mut types = Vec::new();
    let mut at = 8;
    while at + 8 <= png.len() {
        let length = u32::from_be_bytes(png[at..at + 4].try_into().expect("length")) as usize;
        types.push(String::from_utf8_lossy(&png[at + 4..at + 8]).into_owned());
        at += 12 + length;
    }
    types
}

/// One annotation of each of the nine tools, inside a 40 x 40 image.
pub fn every_tool() -> serde_json::Value {
    json!([
        { "type": "rect", "x": 2, "y": 2, "w": 10, "h": 8, "color": "#ff0000", "width": 2 },
        { "type": "arrow", "x1": 1, "y1": 1, "x2": 30, "y2": 20, "color": "#00ff00", "width": 3 },
        { "type": "line", "x1": 0, "y1": 30, "x2": 40, "y2": 30, "color": "#0000ff", "width": 1 },
        { "type": "freehand", "points": [[1, 1], [5, 6], [9, 4]], "color": "#123456", "width": 2 },
        { "type": "text", "x": 4, "y": 4, "text": "Bug here", "color": "#000000", "size": 14 },
        { "type": "marker", "x": 20, "y": 20, "number": 1, "color": "#e53935", "size": 16 },
        { "type": "highlight", "x": 0, "y": 0, "w": 20, "h": 6, "color": "#ffeb3b" },
        { "type": "redact", "x": 10, "y": 10, "w": 16, "h": 16, "mode": "pixelate", "block": 8 },
        { "type": "crop", "x": 0, "y": 0, "w": 40, "h": 40 }
    ])
}

/// A host granting what the manifest grants: `project-read` and `out-write`.
pub fn host() -> FolderHost {
    FolderHost::new(
        MemoryHost::new()
            .grant(Capability::ProjectRead)
            .grant(Capability::OutWrite),
    )
}

/// The pixel at `x`, `y` of straight RGBA `data` `width` pixels wide.
pub fn at(data: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [data[i], data[i + 1], data[i + 2], data[i + 3]]
}

/// [`MemoryHost`] with the real host's listing: a folder's name ends in `/`.
pub struct FolderHost(pub MemoryHost, pub Option<Swap>);

/// A file replaced in place after so many range reads: a capture written again while the
/// editor reads it.
pub struct Swap {
    pub after: usize,
    pub path: String,
    pub bytes: Vec<u8>,
}

impl FolderHost {
    pub fn new(host: MemoryHost) -> Self {
        Self(host, None)
    }

    /// Replaces `path` with `bytes` once `after` range reads are done.
    pub fn swap(mut self, after: usize, path: &str, bytes: &[u8]) -> Self {
        self.1 = Some(Swap {
            after,
            path: path.to_owned(),
            bytes: bytes.to_vec(),
        });
        self
    }

    /// Adds a project file.
    pub fn file(mut self, path: &str, bytes: &[u8]) -> Self {
        self.0.project.insert(path.to_owned(), bytes.to_vec());
        self
    }
}

impl PluginHost for FolderHost {
    fn log(&mut self, level: &str, message: &str) {
        self.0.log(level, message);
    }

    fn now_unix_seconds(&mut self) -> u64 {
        self.0.now_unix_seconds()
    }

    fn setting(&mut self, key: &str) -> Option<String> {
        self.0.setting(key)
    }

    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        self.0.views()
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        self.0.active_view()
    }

    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        self.0.evaluate(view, script)
    }

    fn cdp_send(&mut self, view: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        self.0.cdp_send(view, method, params)
    }

    fn fetch(&mut self, request: &HttpRequest) -> Result<HttpResponse, Failure> {
        self.0.fetch(request)
    }

    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        self.0.project_read(path)
    }

    fn project_read_range(
        &mut self,
        path: &str,
        offset: u64,
        length: u32,
    ) -> Result<(Vec<u8>, u64), Failure> {
        let answer = self.0.project_read_range(path, offset, length);
        if self
            .1
            .as_ref()
            .is_some_and(|swap| self.0.ranges.len() >= swap.after)
            && let Some(swap) = self.1.take()
        {
            self.0.project.insert(swap.path, swap.bytes);
        }
        answer
    }

    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure> {
        let names = self.0.project_list(dir)?;
        let prefix = format!("{}/", dir.trim_end_matches('/'));
        if names.is_empty() && !self.0.project.keys().any(|path| path.starts_with(&prefix)) {
            return Err(Failure::coded(
                rb_plugin_api::FailureKind::NotFound,
                rb_plugin_api::codes::NOT_FOUND,
            ));
        }
        Ok(names
            .into_iter()
            .map(|name| {
                let folder = format!("{prefix}{name}/");
                if self.0.project.keys().any(|path| path.starts_with(&folder)) {
                    format!("{name}/")
                } else {
                    name
                }
            })
            .collect())
    }

    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        self.0.out_write(path, bytes)?;
        // What is written can be read back, as from the real host.
        self.0
            .project
            .insert(format!("{}{path}", crate::paths::OWN_ROOT), bytes.to_vec());
        Ok(())
    }

    fn out_begin(&mut self, path: &str) -> Result<(), Failure> {
        self.0.out_begin(path)
    }

    fn out_append(&mut self, bytes: &[u8]) -> Result<(), Failure> {
        self.0.out_append(bytes)
    }

    fn out_commit(&mut self) -> Result<(), Failure> {
        let staged = self.0.staged.clone();
        self.0.out_commit()?;
        // What is committed can be read back, as from the real host.
        if let Some((path, bytes)) = staged {
            self.0
                .project
                .insert(format!("{}{path}", crate::paths::OWN_ROOT), bytes);
        }
        Ok(())
    }

    fn report_findings(&mut self, items: &[Finding]) -> Result<(), Failure> {
        self.0.report_findings(items)
    }

    fn sidecar_call(&mut self, method: &str, params: &str) -> Result<String, Failure> {
        self.0.sidecar_call(method, params)
    }
}
