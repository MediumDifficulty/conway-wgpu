use std::time::Duration;

use bounded_vec_deque::BoundedVecDeque;
use bytemuck::Pod;
use wgpu::{util::DeviceExt, Device, ShaderStages};

pub struct SimpleUniformHelper<T> {
    inner: T,
    buffer: wgpu::Buffer,
    dirty: bool,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout
}

impl<T> SimpleUniformHelper<T> where T: Pod {
    pub fn from_inner(inner: T, device: &wgpu::Device, visibility: ShaderStages) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&inner),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    count: None,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None
                    },
                    visibility
                }
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            label: None,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding()
                }
            ]
        });

        Self {
            bind_group,
            buffer,
            dirty: false,
            inner,
            bind_group_layout
        }
    }

    pub fn update_inner(&mut self, mut updater: impl FnMut(&mut T))  {
        updater(&mut self.inner);
        self.dirty = true;
    }

    pub fn update_buffer(&self, queue: &wgpu::Queue) {
        if !self.dirty {
            return;
        }

        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&self.inner));
    }

    pub fn update(&mut self, queue: &wgpu::Queue, updater: impl FnMut(&mut T)) {
        self.update_inner(updater);
        self.update_buffer(queue);
    }

    #[inline(always)]
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    #[inline(always)]
    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }
}

pub struct Profiler {
    set: wgpu::QuerySet,
    resolve_buffer: wgpu::Buffer,
    destination_buffer: wgpu::Buffer,
    /// Number of different things which are being timed
    operations: usize,
    /// How many invocations should be used to get an averaged result
    samples: Vec<BoundedVecDeque<u64>>,
    period: f32
}

impl Profiler {
    pub fn new(operations: usize, frame_count: usize, device: &Device, period: f32) -> Self {
        let buffer_size = operations * 2;
        assert!(buffer_size < wgpu::QUERY_SET_MAX_QUERIES as usize, "Maximum time set queries exceeded");

        Self {
            operations,
            set: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: None,
                ty: wgpu::QueryType::Timestamp,
                count: buffer_size as u32,
            }),
            resolve_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                mapped_at_creation: false,
                size: size_of::<u64>() as u64 * buffer_size as u64,
                usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::QUERY_RESOLVE
            }),
            destination_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                mapped_at_creation: false,
                size: size_of::<u64>() as u64 * buffer_size as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ
            }),
            samples: (0..operations)
                .map(|_| BoundedVecDeque::with_capacity(frame_count, frame_count))
                .collect::<Vec<_>>(),
            period
        }
    }

    /// can only be called once all operations have finished
    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.resolve_query_set(
            &self.set,
            0..self.operations as u32 * 2,
            &self.resolve_buffer,
            0
        );
        encoder.copy_buffer_to_buffer(
            &self.resolve_buffer,
            0,
            &self.destination_buffer,
            0,
            self.resolve_buffer.size()
        );
    }

    pub fn process_results(&mut self, device: &wgpu::Device) {
        self.destination_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, |_| ());

        device.poll(wgpu::Maintain::wait()).panic_on_timeout();

        let view = self.destination_buffer
            .slice(..)
            .get_mapped_range();

        let data: Vec<u64> = bytemuck::cast_slice(&view).to_vec();
        drop(view);
        self.destination_buffer.unmap();

        for (i, chunk) in data.chunks_exact(2).enumerate() {
            let [start, stop] = chunk.try_into().unwrap();
            self.samples[i].push_front(stop - start);
        }

    }

    pub fn average_time_raw(&self, idx: usize) -> u64 {
        self.samples[idx].iter().sum::<u64>() / self.samples[idx].len() as u64
    }

    pub fn average_time(&self, idx: usize) -> Duration {
        Duration::from_secs_f64(self.average_time_raw(idx) as f64 * self.period as f64 * 1e-9)
    }

    pub fn compute_pass_timestamp_writes(&self, idx: u32) -> wgpu::ComputePassTimestampWrites {
        let idx = idx * 2;
        wgpu::ComputePassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(idx),
            end_of_pass_write_index: Some(idx + 1),
        }
    }

    pub fn render_pass_timestamp_writes(&self, idx: u32) -> wgpu::RenderPassTimestampWrites {
        let idx = idx * 2;
        wgpu::RenderPassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(idx),
            end_of_pass_write_index: Some(idx + 1),
        }
    }
}