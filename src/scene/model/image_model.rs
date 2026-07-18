// ImageModel — CPU-side data for a raster image quad.
//
// Holds decoded RGBA pixel data and the world-space quad geometry derived
// from the RasterImage entity's insertion point, u/v vectors, and pixel size.

use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ImageModel {
    /// Original file path (used for reload / display in properties).
    pub file_path: String,
    /// RGBA8 pixel data in row-major order. Arc-wrapped so cloning ImageModel
    /// is O(1) — the pixel bytes are shared, not copied.
    pub pixels: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    /// Opacity: 1.0 = opaque, 0.0 = transparent.
    pub opacity: f32,
    /// World-space quad corners (CCW), same order as image_corners() helper:
    ///   [0] origin (bottom-left)
    ///   [1] origin + U*W (bottom-right)
    ///   [2] origin + U*W + V*H (top-right)
    ///   [3] origin + V*H (top-left)
    pub corners: [[f32; 3]; 4],
    /// Low residual paired with `corners` (double-single) so the GPU keeps
    /// sub-unit precision at UTM-scale insertion points.
    pub corners_low: [[f32; 3]; 4],
    /// Optional world-space XY rect [x0, y0, x1, y1] for paper-space
    /// viewport clipping. Mirrors `WireModel.vp_scissor` /
    /// `HatchModel.vp_scissor`.
    pub vp_scissor: Option<[f32; 4]>,
    /// Normalized draw-order depth in (0,1); higher draws on top. Fed to the
    /// image pipeline as a small clip-z bias so the raster orders correctly
    /// against other entity types.
    pub draw_depth: f32,
}

impl ImageModel {
    /// Build an ImageModel from a DXF RasterImage entity.
    /// Returns `None` if the image file cannot be opened or decoded.
    pub fn from_raster_image(
        img: &acadrust::entities::RasterImage,
    ) -> Option<Self> {
        let w = img.size.x;
        let h = img.size.y;
        // Model-space geometry is drawn in (WCS - world_offset) so large UTM-
        // scale coordinates stay within f32 precision; offset the image too.
        // Corners come from a large insertion point plus small u/v spans.
        // Split each into double-single (high, low) f32 so the GPU keeps
        // sub-unit precision at UTM scale and after a cross-drawing paste.
        let oxv = img.insertion_point.x;
        let oyv = img.insertion_point.y;
        let ozv = img.insertion_point.z;
        let ux = (img.u_vector.x * w) as f32;
        let uy = (img.u_vector.y * w) as f32;
        let uz = (img.u_vector.z * w) as f32;
        let vx = (img.v_vector.x * h) as f32;
        let vy = (img.v_vector.y * h) as f32;
        let vz = (img.v_vector.z * h) as f32;
        // High/low split of the anchor; the u/v spans are small and added to
        // the high half (their own residual is below f32 noise at this scale).
        let ox = oxv as f32;
        let oy = oyv as f32;
        let oz = ozv as f32;
        let oxl = (oxv - ox as f64) as f32;
        let oyl = (oyv - oy as f64) as f32;
        let ozl = (ozv - oz as f64) as f32;
        let corners = [
            [ox, oy, oz],
            [ox + ux, oy + uy, oz + uz],
            [ox + ux + vx, oy + uy + vy, oz + uz + vz],
            [ox + vx, oy + vy, oz + vz],
        ];
        let corners_low = [[oxl, oyl, ozl]; 4];
        let opacity = 1.0 - img.fade as f32 / 100.0;

        let (pixels, width, height) = load_pixels(&img.file_path)?;
        Some(Self {
            file_path: img.file_path.clone(),
            pixels: Arc::new(pixels),
            width,
            height,
            opacity,
            corners,
            corners_low,
            vp_scissor: None,
            draw_depth: 0.0,
        })
    }
}

impl ImageModel {
    /// Build an ImageModel from an OLE2FRAME's embedded presentation bitmap.
    /// Returns `None` when the frame is degenerate or carries no decodable
    /// bitmap (e.g. a metafile-only OLE), so the caller falls back to the frame
    /// placeholder.
    pub fn from_ole2frame(ole: &acadrust::entities::Ole2Frame) -> Option<Self> {
        let (pixels, width, height) = decode_ole_bitmap(&ole.binary_data)?;

        // Frame rectangle in WCS. `upper_left`/`lower_right` name the diagonal;
        // normalise to left/right/top/bottom so the bitmap sits upright.
        let left = ole.upper_left_corner.x.min(ole.lower_right_corner.x);
        let right = ole.upper_left_corner.x.max(ole.lower_right_corner.x);
        let bottom = ole.upper_left_corner.y.min(ole.lower_right_corner.y);
        let top = ole.upper_left_corner.y.max(ole.lower_right_corner.y);
        let z = ole.upper_left_corner.z;
        if (right - left).abs() < 1e-9 || (top - bottom).abs() < 1e-9 {
            return None;
        }

        // Double-single split per corner so the quad stays precise at UTM scale.
        let split = |x: f64, y: f64| -> ([f32; 3], [f32; 3]) {
            let (hx, hy, hz) = (x as f32, y as f32, z as f32);
            (
                [hx, hy, hz],
                [
                    (x - hx as f64) as f32,
                    (y - hy as f64) as f32,
                    (z - hz as f64) as f32,
                ],
            )
        };
        // corners: [BL, BR, TR, TL] — the image pipeline maps texel (0,0) to TL.
        let (c0, l0) = split(left, bottom);
        let (c1, l1) = split(right, bottom);
        let (c2, l2) = split(right, top);
        let (c3, l3) = split(left, top);

        Some(Self {
            file_path: "OLE2FRAME".to_string(),
            pixels: Arc::new(pixels),
            width,
            height,
            opacity: 1.0,
            corners: [c0, c1, c2, c3],
            corners_low: [l0, l1, l2, l3],
            vp_scissor: None,
            draw_depth: 0.0,
        })
    }
}

/// Extract and decode the presentation BMP embedded in an OLE2FRAME data blob.
/// The blob carries the OLE object's cached bitmap (a `BITMAPFILEHEADER` "BM"
/// followed by a DIB); scan for a self-consistent one and decode it. Returns
/// `None` when no valid BMP is present (a metafile/other-format OLE).
fn decode_ole_bitmap(data: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut i = 0usize;
    while i + 54 <= data.len() {
        if &data[i..i + 2] != b"BM" {
            i += 1;
            continue;
        }
        let file_size = u32::from_le_bytes(data[i + 2..i + 6].try_into().unwrap()) as usize;
        let dib_size = u32::from_le_bytes(data[i + 14..i + 18].try_into().unwrap());
        // A plausible BITMAPFILEHEADER points at a known DIB-header size.
        if !matches!(dib_size, 12 | 40 | 52 | 56 | 64 | 108 | 124) {
            i += 1;
            continue;
        }
        let end = if file_size >= 54 && i + file_size <= data.len() {
            i + file_size
        } else {
            data.len()
        };
        if let Ok(img) = image::load_from_memory_with_format(&data[i..end], image::ImageFormat::Bmp)
        {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            if w > 0 && h > 0 {
                return Some((rgba.into_raw(), w, h));
            }
        }
        i += 1;
    }
    None
}

/// Decode a raster image file into RGBA8 pixels.
/// Returns `None` if the file does not exist or cannot be decoded.
pub fn load_pixels(path_str: &str) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::open(Path::new(path_str)).ok()?;
    // GPUs cap 2-D texture dimensions (8192 with wgpu's default limits).
    // Downscale oversized images to fit, preserving aspect ratio, so texture
    // creation can't fail — they're displayed scaled-down anyway.
    const MAX_DIM: u32 = 8192;
    let img = if img.width() > MAX_DIM || img.height() > MAX_DIM {
        let longest = img.width().max(img.height()) as f32;
        let scale = MAX_DIM as f32 / longest;
        let nw = ((img.width() as f32 * scale) as u32).clamp(1, MAX_DIM);
        let nh = ((img.height() as f32 * scale) as u32).clamp(1, MAX_DIM);
        img.resize(nw, nh, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some((rgba.into_raw(), w, h))
}
