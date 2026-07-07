//! CPU triangulation of hatch boundaries. Each hatch boundary is a flat
//! list of local-space verts with NaN markers separating sub-loops
//! (islands / holes). We split on the NaN markers into contours and fill
//! them with lyon's even-odd rule — identical coverage to the per-fragment
//! `in_polygon` ray-cast in `hatch_batched.wgsl` (including the #140 island
//! fix) — so the rasterized triangles can replace that test entirely.

use lyon_tessellation::math::point;
use lyon_tessellation::path::Path;
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, VertexBuffers,
};

/// Split a NaN-separated boundary into finite contours.
fn boundary_to_contours(boundary: &[[f32; 2]]) -> Vec<Vec<[f32; 2]>> {
    let mut contours = Vec::new();
    let mut cur: Vec<[f32; 2]> = Vec::new();
    for &[x, y] in boundary {
        if x.is_finite() && y.is_finite() {
            cur.push([x, y]);
        } else if !cur.is_empty() {
            contours.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        contours.push(cur);
    }
    contours
}

/// Triangulate a hatch boundary into a flat triangle-list in local space.
/// Empty on degenerate input or tessellation failure (caller falls back to
/// the AABB-quad + `in_polygon` path for that instance).
pub fn tessellate_boundary(boundary: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let contours = boundary_to_contours(boundary);

    let mut builder = Path::builder();
    for contour in &contours {
        if contour.len() < 3 {
            continue;
        }
        builder.begin(point(contour[0][0], contour[0][1]));
        for p in &contour[1..] {
            builder.line_to(point(p[0], p[1]));
        }
        builder.end(true);
    }
    let path = builder.build();

    let mut geometry: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let mut tess = FillTessellator::new();
    if tess
        .tessellate_path(
            &path,
            &FillOptions::default().with_fill_rule(FillRule::EvenOdd),
            &mut BuffersBuilder::new(&mut geometry, |v: FillVertex| v.position().to_array()),
        )
        .is_err()
    {
        return Vec::new();
    }

    let mut tris = Vec::with_capacity(geometry.indices.len());
    for &idx in &geometry.indices {
        if let Some(&p) = geometry.vertices.get(idx as usize) {
            tris.push(p);
        }
    }
    tris
}

#[cfg(test)]
mod tests {
    use super::*;

    /// True if `p` lies inside any triangle of a flat triangle-list.
    fn covered(tris: &[[f32; 2]], p: [f32; 2]) -> bool {
        fn sign(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
            (a[0] - c[0]) * (b[1] - c[1]) - (b[0] - c[0]) * (a[1] - c[1])
        }
        tris.chunks_exact(3).any(|t| {
            let (a, b, c) = (t[0], t[1], t[2]);
            let d1 = sign(p, a, b);
            let d2 = sign(p, b, c);
            let d3 = sign(p, c, a);
            let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
            let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
            !(has_neg && has_pos)
        })
    }

    #[test]
    fn square_tessellates_and_covers_center() {
        let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let tris = tessellate_boundary(&sq);
        assert!(!tris.is_empty(), "square should tessellate");
        assert_eq!(tris.len() % 3, 0, "triangle-list length must be a multiple of 3");
        assert!(covered(&tris, [5.0, 5.0]), "center must be covered");
        assert!(!covered(&tris, [20.0, 20.0]), "outside point must not be covered");
    }

    #[test]
    fn even_odd_hole_is_not_covered() {
        // Outer 0..10 square, NaN separator, inner 3..7 square (a hole).
        let nan = f32::NAN;
        let boundary = [
            [0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0],
            [nan, nan],
            [3.0, 3.0], [7.0, 3.0], [7.0, 7.0], [3.0, 7.0],
        ];
        let tris = tessellate_boundary(&boundary);
        assert!(!tris.is_empty());
        assert!(covered(&tris, [1.0, 1.0]), "point in the ring must be covered");
        assert!(!covered(&tris, [5.0, 5.0]), "point in the hole must NOT be covered (even-odd)");
    }

    #[test]
    fn degenerate_boundary_returns_empty() {
        assert!(tessellate_boundary(&[[0.0, 0.0], [1.0, 1.0]]).is_empty());
        assert!(tessellate_boundary(&[]).is_empty());
    }

    #[test]
    fn nested_island_even_odd_fills_center() {
        // Even-odd: outer ring filled, hole empty, island inside the hole filled again.
        let nan = f32::NAN;
        let boundary = [
            [0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], // outer
            [nan, nan],
            [2.0, 2.0], [8.0, 2.0], [8.0, 8.0], [2.0, 8.0],     // hole
            [nan, nan],
            [3.0, 3.0], [7.0, 3.0], [7.0, 7.0], [3.0, 7.0],     // island inside the hole
        ];
        let tris = tessellate_boundary(&boundary);
        assert!(!tris.is_empty());
        assert!(covered(&tris, [5.0, 5.0]), "innermost island must be filled");
        assert!(!covered(&tris, [2.5, 5.0]), "the hole ring must be empty");
        assert!(covered(&tris, [0.5, 5.0]), "outer ring must be filled");
    }

    #[test]
    fn self_intersecting_boundary_is_well_formed() {
        // Bowtie / figure-8: a self-intersecting single loop. Must not panic and
        // must return a well-formed triangle-list (possibly empty -> caller falls
        // back to the AABB-quad + in_polygon path).
        let bowtie = [[0.0, 0.0], [10.0, 10.0], [0.0, 10.0], [10.0, 0.0]];
        let tris = tessellate_boundary(&bowtie);
        assert_eq!(tris.len() % 3, 0);
    }
}
