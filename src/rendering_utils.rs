use bytemuck::Pod;
use wgpu::{util::DeviceExt, ShaderStages};

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