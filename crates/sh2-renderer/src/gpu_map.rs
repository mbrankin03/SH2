//! GPU-resident static level map mesh buffers (`GpuMap` and `GpuMapBatch`) with consolidated
//! vertex/index buffer packing, material sorting, and `stageparts` sub-mesh group
//! (`part_group` / `variant_index`) visibility filtering (`SH2-78`, `SH2-182`).

use std::ops::Range;

use sh2_assets::formats::map::{BatchGroup, MapFile, MapVertex};
use sh2_core::events::stageparts::MapPartVisibilityFilter;
use wgpu::util::DeviceExt;

use crate::mirror::{PlanarMirrorAxis, PlanarMirrorSpec};

/// Metadata and buffer sub-range for a single static level draw batch inside [`GpuMap`].
///
/// All batches in a [`GpuMap`] share a single consolidated vertex buffer and index buffer
/// (`SH2-182`) and are submitted via `draw_indexed(first_index..first_index + num_indices, base_vertex, 0..1)`.
pub struct GpuMapBatch {
    /// Starting index offset into [`GpuMap::index_buffer`].
    pub first_index: u32,
    /// Number of indices to submit in `draw_indexed`.
    pub num_indices: u32,
    /// Base vertex offset into [`GpuMap::vertex_buffer`] for `draw_indexed`.
    pub base_vertex: i32,
    /// Original batch index inside the source [`MapFile::batches`] before material sorting.
    pub source_batch_index: usize,
    /// Approximate GPU buffer size in bytes (vertex + index contribution) for VRAM residency accounting.
    pub gpu_bytes: usize,
    /// Render queue category (`Opaque`, `Transparent`, or `Decal`).
    pub group: BatchGroup,
    /// Convenience flag matching `BatchGroup::Transparent`.
    pub is_transparent: bool,
    /// Index into the parent [`MapFile::materials`] table.
    pub material_index: usize,
    /// Raw `.map` sub-geometry identifier (`part_group` in bits `8..15`, `variant_index` in bits `0..7`).
    pub geo_id: u16,
    /// Sub-room / part-group offset (`geo_id >> 8`, `sh2pc.exe` `0x005b47a0`).
    pub part_group: u8,
    /// 1-based switchable variant index (`geo_id & 0xff`; `0` = always visible base geometry).
    pub variant_index: u8,
    /// Axis-aligned bounding box minimum `(x, y, z)` in PS2 mm.
    pub bounds_min: [f32; 3],
    /// Axis-aligned bounding box maximum `(x, y, z)` in PS2 mm.
    pub bounds_max: [f32; 3],
}

impl GpuMapBatch {
    /// Returns the overflow-checked index range `first_index..first_index + num_indices` into
    /// [`GpuMap::index_buffer`], or `None` if `first_index + num_indices` would overflow `u32`.
    #[inline]
    #[must_use]
    pub const fn index_range(&self) -> Option<Range<u32>> {
        match self.first_index.checked_add(self.num_indices) {
            Some(end) => Some(self.first_index..end),
            None => None,
        }
    }
}

/// CPU-side packed geometry produced before uploading a [`GpuMap`]'s consolidated GPU buffers.
struct PackedMapGeometry {
    vertices: Vec<MapVertex>,
    indices: Vec<u32>,
    batches: Vec<GpuMapBatch>,
    opaque_range: Range<usize>,
    decal_range: Range<usize>,
    transparent_range: Range<usize>,
    total_gpu_bytes: usize,
}

/// Packs all non-empty batches of `map` into contiguous vertex/index arrays, sorting `Opaque` and
/// `Decal` batches by `(group, material_index)` to minimize texture bind-group switches while
/// strictly preserving the original draw order of `Transparent` batches (`SH2-182`).
fn pack_map_geometry(map: &MapFile) -> PackedMapGeometry {
    let mut order: Vec<(usize, &sh2_assets::formats::map::MapBatch)> = map
        .batches
        .iter()
        .enumerate()
        .filter(|(_, b)| !b.indices.is_empty() && !b.positions.is_empty())
        .collect();

    order.sort_by_key(|&(orig_idx, batch)| match batch.group {
        BatchGroup::Opaque => (0u8, batch.material_index, orig_idx),
        BatchGroup::Decal => (1u8, batch.material_index, orig_idx),
        // Preserve exact PS2/PC relative draw order for alpha-blended transparent geometry.
        BatchGroup::Transparent => (2u8, 0usize, orig_idx),
    });

    let total_verts: usize = order.iter().map(|(_, b)| b.positions.len()).sum();
    let total_indices: usize = order.iter().map(|(_, b)| b.indices.len()).sum();

    let mut vertices = Vec::with_capacity(total_verts);
    let mut indices = Vec::with_capacity(total_indices);
    let mut batches = Vec::with_capacity(order.len());
    let mut total_gpu_bytes = 0usize;

    let mut opaque_end = 0usize;
    let mut decal_end = 0usize;

    for (orig_idx, batch) in order {
        let material = map.materials.get(batch.material_index);
        let interleaved = batch.interleaved_vertices_for_material(material);
        if interleaved.is_empty() || batch.indices.is_empty() {
            continue;
        }
        let Ok(base_vertex) = i32::try_from(vertices.len()) else {
            continue;
        };
        let Ok(first_index) = u32::try_from(indices.len()) else {
            continue;
        };
        let Ok(num_indices) = u32::try_from(batch.indices.len()) else {
            continue;
        };
        if first_index.checked_add(num_indices).is_none() {
            continue;
        }

        let gpu_bytes =
            interleaved.len() * std::mem::size_of::<MapVertex>() + batch.indices.len() * 4;
        total_gpu_bytes += gpu_bytes;

        vertices.extend_from_slice(&interleaved);
        indices.extend_from_slice(&batch.indices);

        let mut bounds_min = [f32::INFINITY; 3];
        let mut bounds_max = [f32::NEG_INFINITY; 3];
        for p in &batch.positions {
            for axis in 0..3 {
                bounds_min[axis] = bounds_min[axis].min(p[axis]);
                bounds_max[axis] = bounds_max[axis].max(p[axis]);
            }
        }

        batches.push(GpuMapBatch {
            first_index,
            num_indices,
            base_vertex,
            source_batch_index: orig_idx,
            gpu_bytes,
            group: batch.group,
            is_transparent: matches!(batch.group, BatchGroup::Transparent),
            material_index: batch.material_index,
            geo_id: batch.geo_id,
            part_group: batch.part_group(),
            variant_index: batch.variant_index(),
            bounds_min,
            bounds_max,
        });

        match batch.group {
            BatchGroup::Opaque => {
                opaque_end = batches.len();
                decal_end = batches.len();
            }
            BatchGroup::Decal => {
                decal_end = batches.len();
            }
            BatchGroup::Transparent => {}
        }
    }

    let opaque_range = 0..opaque_end;
    let decal_range = opaque_end..decal_end;
    let transparent_range = decal_end..batches.len();

    PackedMapGeometry {
        vertices,
        indices,
        batches,
        opaque_range,
        decal_range,
        transparent_range,
        total_gpu_bytes,
    }
}

/// GPU-resident compiled level geometry for a single `.map` file or outdoor tile.
pub struct GpuMap {
    /// Consolidated vertex buffer containing interleaved [`MapVertex`] records for all batches.
    pub vertex_buffer: Option<wgpu::Buffer>,
    /// Consolidated triangle-list `u32` index buffer for all batches.
    pub index_buffer: Option<wgpu::Buffer>,
    /// Compiled GPU draw batches, partitioned into `Opaque`, `Decal`, and `Transparent` ranges.
    pub batches: Vec<GpuMapBatch>,
    /// Slice range of [`BatchGroup::Opaque`] batches inside [`Self::batches`], sorted by `material_index`.
    pub opaque_range: Range<usize>,
    /// Slice range of [`BatchGroup::Decal`] batches inside [`Self::batches`], sorted by `material_index`.
    pub decal_range: Range<usize>,
    /// Slice range of [`BatchGroup::Transparent`] batches inside [`Self::batches`], in original draw order.
    pub transparent_range: Range<usize>,
    /// Number of [`sh2_assets::formats::map::MapMaterial`] entries in the source [`MapFile`].
    ///
    /// Used to verify that paired texture bind-group tables match the `.map` geometry tier
    /// (`Original PS2 / PC` vs `SH2:EE` / `4K AI Master`, `SH2-121`).
    pub material_count: usize,
    /// Base room or outdoor tile numeric ID (e.g. `0x1d` for `cb29.map`, `0x2e` for `cb46.map`).
    pub base_room_id: u16,
    /// Optional per-frame `stageparts` display-control filter (`EvDispControlModelExec`).
    /// When `None`, all batches are rendered.
    pub visibility_filter: Option<MapPartVisibilityFilter>,
    /// Total GPU buffer footprint in bytes across all batches in this map.
    pub total_gpu_bytes: usize,
}

impl GpuMap {
    /// Extracts the numeric base room / outdoor tile ID from a `.map` stem or path
    /// (e.g. `"ob01"` -> `1`, `"bg/cb/cb29.map"` -> `29`, `"ap104"` -> `104`, or `0` when no
    /// trailing digits exist).
    ///
    /// Matches `sh2pc.exe` `FUN_0047d120` (`0x0047d120`), where each loaded map slot stores its
    /// base room ID so `geo_id` part-group offsets (`geo_id >> 8`) resolve to
    /// `base_room_id + part_group` (e.g. `ob01.map` `base_room_id = 1` + `part_group = 2` ->
    /// `EvRoomInit` `0x005a1350` room `0x03`).
    #[must_use]
    pub fn parse_base_room_id_from_map_name(name: &str) -> u16 {
        let file_name = name.rsplit(['/', '\\']).next().unwrap_or(name);
        let stem = file_name
            .strip_suffix(".map")
            .or_else(|| file_name.strip_suffix(".MAP"))
            .unwrap_or(file_name);
        let digits_start = stem
            .rfind(|c: char| !c.is_ascii_digit())
            .map_or(0, |idx| idx + 1);
        stem[digits_start..].parse::<u16>().unwrap_or(0)
    }

    /// Compiles a parsed [`MapFile`] into consolidated GPU-resident vertex and index buffers,
    /// deriving `base_room_id` from [`MapFile::name`] via [`Self::parse_base_room_id_from_map_name`].
    pub fn from_map(device: &wgpu::Device, map: &MapFile) -> Self {
        Self::from_map_with_room_id(
            device,
            map,
            Self::parse_base_room_id_from_map_name(&map.name),
        )
    }

    /// Compiles a parsed [`MapFile`] into consolidated GPU-resident vertex and index buffers
    /// tagged with its numeric `base_room_id` for `stageparts` display-control filtering (`SH2-78`).
    ///
    /// Both textured and untextured `mode 1/2/6` batches have their vertex RGB modulated by
    /// [`sh2_assets::formats::map::MapMaterial::diffuse_rgba_f32`] (`sh2pc.exe` `FUN_005b4940`
    /// `0x005b4940` + `FUN_005b2190` `0x005b2190`, paired with `VS[5]`/`VS[7]` 2x PS2 GS overbright).
    pub fn from_map_with_room_id(device: &wgpu::Device, map: &MapFile, base_room_id: u16) -> Self {
        let packed = pack_map_geometry(map);

        let (vertex_buffer, index_buffer) =
            if !packed.vertices.is_empty() && !packed.indices.is_empty() {
                let vtx_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("GpuMap Consolidated Vertex Buffer"),
                    contents: bytemuck::cast_slice(&packed.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                let idx_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("GpuMap Consolidated Index Buffer"),
                    contents: bytemuck::cast_slice(&packed.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
                (Some(vtx_buf), Some(idx_buf))
            } else {
                (None, None)
            };

        Self {
            vertex_buffer,
            index_buffer,
            batches: packed.batches,
            opaque_range: packed.opaque_range,
            decal_range: packed.decal_range,
            transparent_range: packed.transparent_range,
            material_count: map.materials.len(),
            base_room_id,
            visibility_filter: None,
            total_gpu_bytes: packed.total_gpu_bytes,
        }
    }

    /// Returns the contiguous slice of batches belonging to `group`.
    #[inline]
    #[must_use]
    pub fn batches_for_group(&self, group: BatchGroup) -> &[GpuMapBatch] {
        let range = match group {
            BatchGroup::Opaque => &self.opaque_range,
            BatchGroup::Decal => &self.decal_range,
            BatchGroup::Transparent => &self.transparent_range,
        };
        &self.batches[range.clone()]
    }

    /// Resolves the mirror decal batch referenced by `spec.decal_batch_index`, supporting both
    /// indices into [`Self::batches`] (from [`PlanarMirrorSpec::for_room_and_gpu_map`]) and
    /// original [`MapFile::batches`] indices (from [`PlanarMirrorSpec::for_room_and_map`]).
    #[must_use]
    pub fn resolve_mirror_decal_batch(&self, spec: &PlanarMirrorSpec) -> Option<&GpuMapBatch> {
        let batch_idx = spec.decal_batch_index?;
        const EPS_MM: f32 = 1.0;
        let comp = match spec.axis {
            PlanarMirrorAxis::AxisX => 0,
            PlanarMirrorAxis::AxisY => 1,
            PlanarMirrorAxis::AxisZ => 2,
        };
        let wall = spec.wall_pos();
        let matches_plane = |b: &GpuMapBatch| {
            b.group == BatchGroup::Decal
                && b.num_indices > 0
                && (b.bounds_min[comp] - wall).abs() <= EPS_MM
                && (b.bounds_max[comp] - wall).abs() <= EPS_MM
        };

        if let Some(batch) = self.batches.get(batch_idx) {
            if matches_plane(batch) {
                return Some(batch);
            }
        }
        self.batches_for_group(BatchGroup::Decal)
            .iter()
            .find(|b| b.source_batch_index == batch_idx && matches_plane(b))
    }

    /// Returns `true` if every compiled batch's `material_index` is strictly within `0..bind_group_count`.
    #[inline]
    #[must_use]
    pub fn all_batches_within_material_table(&self, bind_group_count: usize) -> bool {
        self.batches
            .iter()
            .all(|b| b.material_index < bind_group_count)
    }

    /// Installs or clears the active per-frame `stageparts` visibility filter.
    pub fn set_visibility_filter(&mut self, filter: Option<MapPartVisibilityFilter>) {
        self.visibility_filter = filter;
    }

    /// Evaluates whether `batch` should be drawn under `self.visibility_filter` or, if none is set
    /// on this map, the active per-context `context_filter` (`RenderContext::set_map_visibility_filter`).
    ///
    /// Exact 1:1 parity with `sh2pc.exe` `0x0047d120` + `0x005b4760` (`SetDispOnOffMask` `0x005b47a0`):
    /// - When neither filter is set, every batch is visible.
    /// - When a filter is active, `variant_index == 0` is always visible and `variant_index > 0`
    ///   is visible iff bit `variant_index - 1` is set in the mask for `base_room_id + part_group`.
    #[inline]
    #[must_use]
    pub fn is_batch_visible_with(
        &self,
        batch: &GpuMapBatch,
        context_filter: Option<&MapPartVisibilityFilter>,
    ) -> bool {
        match self.visibility_filter.as_ref().or(context_filter) {
            Some(filter) => {
                filter.is_batch_visible(self.base_room_id, batch.part_group, batch.variant_index)
            }
            None => true,
        }
    }

    /// Evaluates whether `batch` should be drawn under the currently active `visibility_filter`.
    ///
    /// Exact 1:1 parity with `sh2pc.exe` `0x005b4760` (`SetDispOnOffMask`):
    /// - When `visibility_filter` is `None`, every batch is visible.
    /// - When `Some(filter)`, `variant_index == 0` is always visible and `variant_index > 0`
    ///   is visible iff bit `variant_index - 1` is set in the mask for `base_room_id + part_group`.
    #[inline]
    #[must_use]
    pub fn is_batch_visible(&self, batch: &GpuMapBatch) -> bool {
        self.is_batch_visible_with(batch, None)
    }

    /// Returns the number of batches currently visible under [`Self::visibility_filter`] or
    /// `context_filter`.
    #[must_use]
    pub fn visible_batch_count_with(
        &self,
        context_filter: Option<&MapPartVisibilityFilter>,
    ) -> usize {
        self.batches
            .iter()
            .filter(|b| self.is_batch_visible_with(b, context_filter))
            .count()
    }

    /// Returns the number of batches currently visible under [`Self::visibility_filter`].
    #[must_use]
    pub fn visible_batch_count(&self) -> usize {
        self.visible_batch_count_with(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sh2_assets::formats::map::{MapBatch, MapFile, MapMaterial};

    fn make_triangle_batch(group: BatchGroup, material_index: usize, x_offset: f32) -> MapBatch {
        MapBatch {
            group,
            material_index,
            geo_id: 0,
            positions: vec![
                [x_offset, 0.0, 0.0],
                [x_offset + 10.0, 0.0, 0.0],
                [x_offset, 10.0, 0.0],
            ],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            colors: vec![[255, 255, 255, 255]; 3],
            indices: vec![0, 1, 2],
        }
    }

    fn make_test_map() -> MapFile {
        let default_mat = MapMaterial {
            mode: 1,
            texture_id: 1,
            color: 0xffff_ffff,
            overlay: 0,
            specularity: 0.0,
        };
        MapFile {
            name: "er01.map".to_string(),
            materials: vec![default_mat; 6],
            batches: vec![
                // Transparent batches intentionally out of material order: 5, 1, 4
                make_triangle_batch(BatchGroup::Transparent, 5, 100.0),
                // Opaque batches out of material order: 3, 1, 3, 0
                make_triangle_batch(BatchGroup::Opaque, 3, 0.0),
                make_triangle_batch(BatchGroup::Decal, 4, 200.0),
                make_triangle_batch(BatchGroup::Opaque, 1, 10.0),
                make_triangle_batch(BatchGroup::Transparent, 1, 110.0),
                make_triangle_batch(BatchGroup::Decal, 2, -20_000.0),
                make_triangle_batch(BatchGroup::Opaque, 3, 20.0),
                make_triangle_batch(BatchGroup::Transparent, 4, 120.0),
                make_triangle_batch(BatchGroup::Opaque, 0, 30.0),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn pack_map_geometry_sorts_opaque_and_decal_by_material_and_preserves_transparent_order() {
        let map = make_test_map();
        let packed = pack_map_geometry(&map);

        assert_eq!(packed.batches.len(), 9);
        assert_eq!(packed.opaque_range, 0..4);
        assert_eq!(packed.decal_range, 4..6);
        assert_eq!(packed.transparent_range, 6..9);

        // Opaque sorted by material_index: 0, 1, 3, 3 (stable by source_batch_index: 1 before 6)
        let opaque_mats: Vec<usize> = packed.batches[packed.opaque_range.clone()]
            .iter()
            .map(|b| b.material_index)
            .collect();
        assert_eq!(opaque_mats, vec![0, 1, 3, 3]);
        assert_eq!(packed.batches[2].source_batch_index, 1);
        assert_eq!(packed.batches[3].source_batch_index, 6);

        // Decal sorted by material_index: 2, 4
        let decal_mats: Vec<usize> = packed.batches[packed.decal_range.clone()]
            .iter()
            .map(|b| b.material_index)
            .collect();
        assert_eq!(decal_mats, vec![2, 4]);

        // Transparent preserves exact original order: 5, 1, 4 (source indices 0, 4, 7)
        let trans_mats: Vec<usize> = packed.batches[packed.transparent_range.clone()]
            .iter()
            .map(|b| b.material_index)
            .collect();
        let trans_src: Vec<usize> = packed.batches[packed.transparent_range.clone()]
            .iter()
            .map(|b| b.source_batch_index)
            .collect();
        assert_eq!(trans_mats, vec![5, 1, 4]);
        assert_eq!(trans_src, vec![0, 4, 7]);
    }

    #[test]
    fn pack_map_geometry_assigns_contiguous_base_vertex_and_first_index() {
        let map = make_test_map();
        let packed = pack_map_geometry(&map);

        assert_eq!(packed.vertices.len(), 9 * 3);
        assert_eq!(packed.indices.len(), 9 * 3);

        for (i, batch) in packed.batches.iter().enumerate() {
            assert_eq!(batch.base_vertex, (i * 3) as i32);
            assert_eq!(batch.first_index, (i * 3) as u32);
            assert_eq!(batch.num_indices, 3);
            // Verify the indexed vertices match the batch's bounds_min[0]
            let v0_idx = (batch.base_vertex as usize)
                + (packed.indices[batch.first_index as usize] as usize);
            assert_eq!(packed.vertices[v0_idx].position[0], batch.bounds_min[0]);
        }

        let expected_bytes =
            packed.vertices.len() * std::mem::size_of::<MapVertex>() + packed.indices.len() * 4;
        assert_eq!(packed.total_gpu_bytes, expected_bytes);
    }

    #[test]
    fn resolve_mirror_decal_batch_handles_both_map_and_gpu_map_indices() {
        let mut map = make_test_map();
        // Make source batch 5 a flat mirror decal on X = -20000.0
        map.batches[5].positions = vec![
            [-20_000.0, -931.0, 20_350.0],
            [-20_000.0, -661.0, 20_350.0],
            [-20_000.0, -661.0, 20_569.0],
        ];
        let packed = pack_map_geometry(&map);
        let gpu_map = GpuMap {
            vertex_buffer: None,
            index_buffer: None,
            batches: packed.batches,
            opaque_range: packed.opaque_range,
            decal_range: packed.decal_range,
            transparent_range: packed.transparent_range,
            material_count: map.materials.len(),
            base_room_id: 1,
            visibility_filter: None,
            total_gpu_bytes: packed.total_gpu_bytes,
        };

        // Spec from `for_room_and_map` carries original source index 5
        let spec_from_map = PlanarMirrorSpec::for_room_and_map(0x01, &map).expect("er01 spec");
        assert_eq!(spec_from_map.decal_batch_index, Some(5));
        let resolved_map = gpu_map
            .resolve_mirror_decal_batch(&spec_from_map)
            .expect("must resolve source_batch_index after sorting");
        assert_eq!(resolved_map.source_batch_index, 5);

        // Spec from `for_room_and_gpu_map` carries sorted GpuMap index 4
        let spec_from_gpu =
            PlanarMirrorSpec::for_room_and_gpu_map(0x01, &gpu_map).expect("er01 gpu spec");
        assert_eq!(spec_from_gpu.decal_batch_index, Some(4));
        let resolved_gpu = gpu_map
            .resolve_mirror_decal_batch(&spec_from_gpu)
            .expect("must resolve sorted GpuMap index");
        assert_eq!(resolved_gpu.source_batch_index, 5);
    }

    #[test]
    fn gpu_map_batch_index_range_checks_u32_overflow() {
        let mut batch = GpuMapBatch {
            first_index: 12,
            num_indices: 6,
            base_vertex: 4,
            source_batch_index: 0,
            gpu_bytes: 0,
            group: BatchGroup::Opaque,
            is_transparent: false,
            material_index: 0,
            geo_id: 0,
            part_group: 0,
            variant_index: 0,
            bounds_min: [0.0; 3],
            bounds_max: [1.0; 3],
        };
        assert_eq!(batch.index_range(), Some(12..18));
        batch.first_index = u32::MAX - 2;
        batch.num_indices = 6;
        assert_eq!(batch.index_range(), None);
    }
}
