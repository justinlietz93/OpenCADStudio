//! Shared spatial broad phase for cursor and area interactions.
//!
//! Exact snap and selection rules remain in their consumers. This index only
//! narrows a large, camera-independent Model wire set to wire and segment
//! candidates whose world-XY bounds overlap the interaction aperture.

use crate::scene::model::wire_model::WireModel;
use std::sync::Arc;

const TARGET_ENTRIES_PER_CELL: usize = 8;
const MAX_SPAN_CELLS: u64 = 64;
const MAX_AXIS_CELLS: u32 = 8_192;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SegmentRef {
    pub wire: u32,
    pub start: u32,
}

#[derive(Clone, Copy)]
struct Entry<T> {
    aabb: [f64; 4],
    value: T,
}

struct SpatialGrid<T> {
    min: [f64; 2],
    cell: f64,
    cols: u32,
    rows: u32,
    entries: Vec<Entry<T>>,
    cells: Vec<Vec<u32>>,
    oversized: Vec<u32>,
}

impl<T: Copy + Ord> SpatialGrid<T> {
    fn build(entries: Vec<Entry<T>>) -> Self {
        if entries.is_empty() {
            return Self {
                min: [0.0; 2],
                cell: 1.0,
                cols: 1,
                rows: 1,
                entries,
                cells: vec![Vec::new()],
                oversized: Vec::new(),
            };
        }

        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for entry in &entries {
            min[0] = min[0].min(entry.aabb[0]);
            min[1] = min[1].min(entry.aabb[1]);
            max[0] = max[0].max(entry.aabb[2]);
            max[1] = max[1].max(entry.aabb[3]);
        }
        let ext_x = (max[0] - min[0]).max(1e-9);
        let ext_y = (max[1] - min[1]).max(1e-9);
        let target_cells = ((entries.len() / TARGET_ENTRIES_PER_CELL).max(1)) as f64;
        let cell = ((ext_x * ext_y) / target_cells)
            .sqrt()
            .max(ext_x / (MAX_AXIS_CELLS - 1) as f64)
            .max(ext_y / (MAX_AXIS_CELLS - 1) as f64)
            .max(1e-9);
        let cols = (((ext_x / cell).ceil() as u64) + 1).clamp(1, MAX_AXIS_CELLS as u64) as u32;
        let rows = (((ext_y / cell).ceil() as u64) + 1).clamp(1, MAX_AXIS_CELLS as u64) as u32;
        let mut cells = vec![Vec::new(); cols as usize * rows as usize];
        let mut oversized = Vec::new();

        let col_of = |x: f64| (((x - min[0]) / cell).floor()).clamp(0.0, (cols - 1) as f64) as u32;
        let row_of = |y: f64| (((y - min[1]) / cell).floor()).clamp(0.0, (rows - 1) as f64) as u32;

        for (idx, entry) in entries.iter().enumerate() {
            let c0 = col_of(entry.aabb[0]);
            let c1 = col_of(entry.aabb[2]);
            let r0 = row_of(entry.aabb[1]);
            let r1 = row_of(entry.aabb[3]);
            let span = (c1 - c0 + 1) as u64 * (r1 - r0 + 1) as u64;
            if span > MAX_SPAN_CELLS {
                oversized.push(idx as u32);
                continue;
            }
            for row in r0..=r1 {
                let base = row as usize * cols as usize;
                for col in c0..=c1 {
                    cells[base + col as usize].push(idx as u32);
                }
            }
        }

        Self {
            min,
            cell,
            cols,
            rows,
            entries,
            cells,
            oversized,
        }
    }

    fn query(&self, query: [f64; 4]) -> Vec<T> {
        let mut entry_indices = self.oversized.clone();
        let grid_max_x = self.min[0] + self.cols as f64 * self.cell;
        let grid_max_y = self.min[1] + self.rows as f64 * self.cell;
        if query[2] >= self.min[0]
            && query[3] >= self.min[1]
            && query[0] <= grid_max_x
            && query[1] <= grid_max_y
        {
            let col = |x: f64| {
                (((x - self.min[0]) / self.cell).floor()).clamp(0.0, (self.cols - 1) as f64) as u32
            };
            let row = |y: f64| {
                (((y - self.min[1]) / self.cell).floor()).clamp(0.0, (self.rows - 1) as f64) as u32
            };
            for r in row(query[1])..=row(query[3]) {
                let base = r as usize * self.cols as usize;
                for c in col(query[0])..=col(query[2]) {
                    entry_indices.extend_from_slice(&self.cells[base + c as usize]);
                }
            }
        }

        entry_indices.sort_unstable();
        entry_indices.dedup();
        let mut out: Vec<T> = entry_indices
            .into_iter()
            .filter_map(|idx| {
                let entry = self.entries.get(idx as usize)?;
                aabb_overlaps(entry.aabb, query).then_some(entry.value)
            })
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }
}

pub struct InteractionIndex {
    wires: SpatialGrid<u32>,
    segments: SpatialGrid<SegmentRef>,
    unbounded_wires: Vec<u32>,
}

pub struct InteractionHandleIndex {
    handles: SpatialGrid<u64>,
}

impl InteractionHandleIndex {
    pub fn build(entries: impl IntoIterator<Item = (u64, [f64; 4])>) -> Self {
        Self {
            handles: SpatialGrid::build(
                entries
                    .into_iter()
                    .filter(|(_, aabb)| {
                        aabb.iter().all(|value| value.is_finite())
                            && aabb[0] <= aabb[2]
                            && aabb[1] <= aabb[3]
                    })
                    .map(|(value, aabb)| Entry { aabb, value })
                    .collect(),
            ),
        }
    }

    pub fn query(&self, aabb: [f64; 4]) -> Vec<u64> {
        self.handles.query(aabb)
    }
}

impl InteractionIndex {
    pub fn build(wires: &[WireModel]) -> Self {
        let mut wire_entries = Vec::with_capacity(wires.len());
        let mut segment_entries = Vec::new();
        let mut unbounded_wires = Vec::new();

        for (wire_idx, wire) in wires.iter().enumerate() {
            if let Some(aabb) = finite_wire_aabb(wire) {
                wire_entries.push(Entry {
                    aabb,
                    value: wire_idx as u32,
                });
            } else {
                unbounded_wires.push(wire_idx as u32);
            }

            for start in 0..wire.points.len().saturating_sub(1) {
                let a = wire_point(wire, start);
                let b = wire_point(wire, start + 1);
                if !a[0].is_finite() || !a[1].is_finite() || !b[0].is_finite() || !b[1].is_finite()
                {
                    continue;
                }
                segment_entries.push(Entry {
                    aabb: [
                        a[0].min(b[0]),
                        a[1].min(b[1]),
                        a[0].max(b[0]),
                        a[1].max(b[1]),
                    ],
                    value: SegmentRef {
                        wire: wire_idx as u32,
                        start: start as u32,
                    },
                });
            }
        }

        Self {
            wires: SpatialGrid::build(wire_entries),
            segments: SpatialGrid::build(segment_entries),
            unbounded_wires,
        }
    }

    pub fn query(&self, wires: Arc<Vec<WireModel>>, aabb: [f64; 4]) -> InteractionCandidates {
        let mut wire_indices = self.wires.query(aabb);
        wire_indices.extend_from_slice(&self.unbounded_wires);
        wire_indices.sort_unstable();
        wire_indices.dedup();
        InteractionCandidates {
            wires,
            wire_indices: Some(wire_indices),
            segments: Some(self.segments.query(aabb)),
            query_aabb: Some(aabb),
        }
    }
}

pub struct InteractionCandidates {
    wires: Arc<Vec<WireModel>>,
    wire_indices: Option<Vec<u32>>,
    segments: Option<Vec<SegmentRef>>,
    query_aabb: Option<[f64; 4]>,
}

pub trait WireSource {
    fn iter(&self) -> WireIter<'_>;
    fn len(&self) -> usize;
    fn get(&self, index: usize) -> Option<&WireModel>;
    fn segments(&self) -> Option<&[SegmentRef]> {
        None
    }
    fn source_wire(&self, index: u32) -> Option<&WireModel>;
}

impl InteractionCandidates {
    pub fn all(wires: Arc<Vec<WireModel>>) -> Self {
        Self {
            wires,
            wire_indices: None,
            segments: None,
            query_aabb: None,
        }
    }

    pub fn iter(&self) -> WireIter<'_> {
        match &self.wire_indices {
            Some(indices) => WireIter::Indexed {
                wires: &self.wires,
                indices: indices.iter(),
            },
            None => WireIter::All(self.wires.as_slice().iter()),
        }
    }

    pub fn len(&self) -> usize {
        self.wire_indices
            .as_ref()
            .map_or(self.wires.len(), Vec::len)
    }

    pub fn get(&self, index: usize) -> Option<&WireModel> {
        match &self.wire_indices {
            Some(indices) => indices
                .get(index)
                .and_then(|&wire| self.wires.get(wire as usize)),
            None => self.wires.get(index),
        }
    }

    pub fn segments(&self) -> Option<&[SegmentRef]> {
        self.segments.as_deref()
    }

    pub fn source_wire(&self, index: u32) -> Option<&WireModel> {
        self.wires.get(index as usize)
    }

    pub fn query_aabb(&self) -> Option<[f64; 4]> {
        self.query_aabb
    }
}

impl WireSource for InteractionCandidates {
    fn iter(&self) -> WireIter<'_> {
        self.iter()
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn get(&self, index: usize) -> Option<&WireModel> {
        self.get(index)
    }

    fn segments(&self) -> Option<&[SegmentRef]> {
        self.segments()
    }

    fn source_wire(&self, index: u32) -> Option<&WireModel> {
        self.source_wire(index)
    }
}

impl WireSource for [WireModel] {
    fn iter(&self) -> WireIter<'_> {
        WireIter::All(<[WireModel]>::iter(self))
    }

    fn len(&self) -> usize {
        <[WireModel]>::len(self)
    }

    fn get(&self, index: usize) -> Option<&WireModel> {
        <[WireModel]>::get(self, index)
    }

    fn source_wire(&self, index: u32) -> Option<&WireModel> {
        self.get(index as usize)
    }
}

impl WireSource for Vec<WireModel> {
    fn iter(&self) -> WireIter<'_> {
        WireIter::All(self.as_slice().iter())
    }

    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn get(&self, index: usize) -> Option<&WireModel> {
        self.as_slice().get(index)
    }

    fn source_wire(&self, index: u32) -> Option<&WireModel> {
        self.get(index as usize)
    }
}

impl<const N: usize> WireSource for [WireModel; N] {
    fn iter(&self) -> WireIter<'_> {
        WireIter::All(self.as_slice().iter())
    }

    fn len(&self) -> usize {
        N
    }

    fn get(&self, index: usize) -> Option<&WireModel> {
        self.as_slice().get(index)
    }

    fn source_wire(&self, index: u32) -> Option<&WireModel> {
        self.get(index as usize)
    }
}

#[derive(Clone)]
pub enum WireIter<'a> {
    All(std::slice::Iter<'a, WireModel>),
    Indexed {
        wires: &'a [WireModel],
        indices: std::slice::Iter<'a, u32>,
    },
}

impl<'a> Iterator for WireIter<'a> {
    type Item = &'a WireModel;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::All(iter) => iter.next(),
            Self::Indexed { wires, indices } => {
                indices.next().and_then(|&idx| wires.get(idx as usize))
            }
        }
    }
}

fn finite_wire_aabb(wire: &WireModel) -> Option<[f64; 4]> {
    let [min_x, min_y, max_x, max_y] = wire.aabb;
    (min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite()).then(|| {
        let mag = wire.aabb.iter().fold(0.0f32, |m, c| m.max(c.abs()));
        let pad = (mag * f32::EPSILON * 2.0) as f64;
        let mut aabb = [
            min_x as f64 - pad,
            min_y as f64 - pad,
            max_x as f64 + pad,
            max_y as f64 + pad,
        ];
        // Some valid snap points are outside visible curve bounds: notably the
        // center of a short arc. Keep those discoverable without widening the
        // segment query used by Intersection/Nearest.
        for (point, _) in &wire.snap_pts {
            if point.x.is_finite() && point.y.is_finite() {
                aabb[0] = aabb[0].min(point.x);
                aabb[1] = aabb[1].min(point.y);
                aabb[2] = aabb[2].max(point.x);
                aabb[3] = aabb[3].max(point.y);
            }
        }
        aabb
    })
}

fn wire_point(wire: &WireModel, index: usize) -> [f64; 3] {
    let high = wire.points[index];
    let low = wire.points_low.get(index).copied().unwrap_or([0.0; 3]);
    [
        high[0] as f64 + low[0] as f64,
        high[1] as f64 + low[1] as f64,
        high[2] as f64 + low[2] as f64,
    ]
}

fn aabb_overlaps(a: [f64; 4], b: [f64; 4]) -> bool {
    a[2] >= b[0] && a[0] <= b[2] && a[3] >= b[1] && a[1] <= b[3]
}
