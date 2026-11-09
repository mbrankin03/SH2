//! GPU-resident static level map mesh buffers (`GpuMap` and `GpuMapBatch`).

use sh2_assets::formats::map::MapFile;
use wgpu::util::DeviceExt;

/// GPU-resident static mesh buffers compiled once during map loading to avoid per-frame allocations.
pub struct GpuMapBatch {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
    pub group: sh2_assets::formats::map::BatchGroup,
    pub is_transparent: bool,
    pub material_index: usize,
}

pub struct GpuMap {
    pub batches: Vec<GpuMapBatch>,
}

impl GpuMap {
    pub fn from_map(device: &wgpu::Device, map: &MapFile) -> Self {
        let mut batches = Vec::with_capacity(map.batches.len());
        for batch in &map.batches {
            let interleaved = batch.interleaved_vertices();
            if batch.indices.is_empty() || interleaved.is_empty() {
                continue;
            }
            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("GpuMap Batch Vertex Buffer"),
                contents: bytemuck::cast_slice(&interleaved),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("GpuMap Batch Index Buffer"),
                contents: bytemuck::cast_slice(&batch.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            batches.push(GpuMapBatch {
                vertex_buffer,
                index_buffer,
                num_indices: batch.indices.len() as u32,
                group: batch.group,
                is_transparent: matches!(
                    batch.group,
                    sh2_assets::formats::map::BatchGroup::Transparent
                ),
                material_index: batch.material_index,
            });
        }
        Self { batches }
    }
}
