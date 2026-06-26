//! Small pure helpers split out of `update.rs`.

use crate::scene::Scene;

/// Parse a scale string like "1:50" or "2:1" into (numerator, denominator).
/// Returns (1.0, 1.0) for "Fit" or unknown formats.
/// Sync the model-space annotation scale into the standard CANNOSCALE /
/// CANNOSCALEVALUE header variables before a save, so the scale round-trips
/// through the file (and is read correctly by other CAD applications).
pub(super) fn sync_annotation_scale_header(scene: &mut Scene) {
    let anno = scene.annotation_scale;
    let value = if anno.abs() > 1e-9 {
        1.0 / anno as f64
    } else {
        1.0
    };
    // Prefer the name of a matching scale already in the drawing's list;
    // fall back to a formatted ratio when none matches.
    let name = scene
        .scale_list()
        .into_iter()
        .find(|(_, a, _)| (a - anno).abs() < 0.001 * anno.max(0.001))
        .map(|(n, _, _)| n)
        .unwrap_or_else(|| format_annotation_scale_name(anno));
    let hdr = &mut scene.document.header;
    hdr.current_annotation_scale = name;
    hdr.annotation_scale_value = value;
}

/// Format an annotation-scale multiplier as a ratio name: 50.0 -> "1:50",
/// 0.5 -> "2:1", 1.0 -> "1:1".
fn format_annotation_scale_name(anno: f32) -> String {
    if anno >= 1.0 {
        format!("1:{}", anno.round() as i64)
    } else if anno > 0.0 {
        format!("{}:1", (1.0 / anno).round() as i64)
    } else {
        "1:1".to_string()
    }
}

pub(super) fn parse_plot_scale(s: &str) -> (f64, f64) {
    if s == "Fit" {
        return (1.0, 1.0);
    }
    if let Some((a, b)) = s.split_once(':') {
        let num: f64 = a.trim().parse().unwrap_or(1.0);
        let den: f64 = b.trim().parse().unwrap_or(1.0);
        if den > 0.0 {
            return (num, den);
        }
    }
    (1.0, 1.0)
}

/// Convert an internal `[r,g,b,a]` colour (0.0–1.0) to a persisted 0–255 RGB
/// triplet, dropping alpha (backgrounds are always opaque).
pub(super) fn f4_to_u3([r, g, b, _]: [f32; 4]) -> [u8; 3] {
    [
        (r * 255.0).round().clamp(0.0, 255.0) as u8,
        (g * 255.0).round().clamp(0.0, 255.0) as u8,
        (b * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

/// Inverse of [`f4_to_u3`]: a persisted 0–255 RGB triplet back to an opaque
/// `[r,g,b,a]` colour.
pub(super) fn u3_to_f4([r, g, b]: [u8; 3]) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

