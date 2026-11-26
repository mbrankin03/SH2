//! GPU-resident static level map mesh buffers (`GpuMap` and `GpuMapBatch`) with `stageparts`
//! sub-mesh group (`part_group` / `variant_index`) visibility filtering (`SH2-78`).

use sh2_assets::formats::map::MapFile;
use sh2_core::events::stageparts::MapPartVisibilityFilter;
use wgpu::util::DeviceExt;

/// GPU-resident static mesh buffers compiled once during map loading to avoid per-frame allocations.
pub struct GpuMapBatch {
    /// Vertex buffer containing interleaved [`sh2_assets::formats::map::MapVertex`] records.
    pub vertex_buffer: wgpu::Buffer,
    /// Triangle-list `u32` index buffer.
    pub index_buffer: wgpu::Buffer,
    /// Number of indices to submit in `draw_indexed`.
    pub num_indices: u32,
    /// Approximate GPU buffer size in bytes (vertex + index buffers) for VRAM residency accounting.
    pub gpu_bytes: usize,
    /// Render queue category (`Opaque`, `Transparent`, or `Decal`).
    pub group: sh2_assets::formats::map::BatchGroup,
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

/// GPU-resident compiled level geometry for a single `.map` file or outdoor tile.
pub struct GpuMap {
    /// Compiled GPU draw batches.
    pub batches: Vec<GpuMapBatch>,
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

    /// Compiles a parsed [`MapFile`] into GPU-resident vertex and index buffers, deriving
    /// `base_room_id` from [`MapFile::name`] via [`Self::parse_base_room_id_from_map_name`].
    pub fn from_map(device: &wgpu::Device, map: &MapFile) -> Self {
        Self::from_map_with_room_id(
            device,
            map,
            Self::parse_base_room_id_from_map_name(&map.name),
        )
    }

    /// Compiles a parsed [`MapFile`] into GPU-resident vertex and index buffers tagged with its
    /// numeric `base_room_id` for `stageparts` display-control filtering (`SH2-78`).
    ///
    /// Untextured batches (`MapMaterial::texture_id == 0`) have their vertex RGB modulated by
    /// [`sh2_assets::formats::map::MapMaterial::diffuse_rgba_f32`] (`sh2pc.exe` `FUN_005b2190`
    /// `0x005b2190`, `SH2-121`).
    pub fn from_map_with_room_id(device: &wgpu::Device, map: &MapFile, base_room_id: u16) -> Self {
        let mut batches = Vec::with_capacity(map.batches.len());
        let mut total_gpu_bytes = 0usize;

        for batch in &map.batches {
            let material = map.materials.get(batch.material_index);
            let interleaved = batch.interleaved_vertices_for_material(material);
            if batch.indices.is_empty() || interleaved.is_empty() {
                continue;
            }
            let vtx_bytes: &[u8] = bytemuck::cast_slice(&interleaved);
            let idx_bytes: &[u8] = bytemuck::cast_slice(&batch.indices);
            let gpu_bytes = vtx_bytes.len() + idx_bytes.len();
            total_gpu_bytes += gpu_bytes;

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("GpuMap Batch Vertex Buffer"),
                contents: vtx_bytes,
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("GpuMap Batch Index Buffer"),
                contents: idx_bytes,
                usage: wgpu::BufferUsages::INDEX,
            });
            let mut bounds_min = [f32::INFINITY; 3];
            let mut bounds_max = [f32::NEG_INFINITY; 3];
            for p in &batch.positions {
                for axis in 0..3 {
                    bounds_min[axis] = bounds_min[axis].min(p[axis]);
                    bounds_max[axis] = bounds_max[axis].max(p[axis]);
                }
            }
            batches.push(GpuMapBatch {
                vertex_buffer,
                index_buffer,
                num_indices: batch.indices.len() as u32,
                gpu_bytes,
                group: batch.group,
                is_transparent: matches!(
                    batch.group,
                    sh2_assets::formats::map::BatchGroup::Transparent
                ),
                material_index: batch.material_index,
                geo_id: batch.geo_id,
                part_group: batch.part_group(),
                variant_index: batch.variant_index(),
                bounds_min,
                bounds_max,
            });
        }
        Self {
            batches,
            material_count: map.materials.len(),
            base_room_id,
            visibility_filter: None,
            total_gpu_bytes,
        }
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
