use super::{Frame, Options};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{sync::mpsc, time::Duration};
use wgpu::util::DeviceExt;

fn instance() -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    })
}
pub fn adapters() -> Value {
    json!(pollster::block_on(instance().enumerate_adapters(wgpu::Backends::PRIMARY)).into_iter().map(|a| {let i=a.get_info();json!({"name":i.name,"backend":format!("{:?}",i.backend),"vendor":i.vendor,"kind":format!("{:?}",i.device_type)})}).collect::<Vec<_>>())
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    dimensions: [u32; 4],
    effects: [f32; 4],
    local: [f32; 4],
    aspect: [f32; 4],
    flags: [u32; 4],
}
struct Buffers {
    key: (u32, u32, u32, u32),
    input: wgpu::Buffer,
    _blur_a: wgpu::Buffer,
    _blur_b: wgpu::Buffer,
    output: wgpu::Buffer,
    readback: wgpu::Buffer,
    group: wgpu::BindGroup,
}
pub struct Gpu {
    pub name: String,
    device: wgpu::Device,
    queue: wgpu::Queue,
    params: wgpu::Buffer,
    mask: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    pipelines: [wgpu::ComputePipeline; 3],
    buffers: Option<Buffers>,
}
impl Gpu {
    pub fn new(name: &str) -> Result<Self> {
        let instance = instance();
        let mut adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::PRIMARY));
        adapters.sort_by_key(|a| match a.get_info().device_type {
            wgpu::DeviceType::DiscreteGpu => 0,
            wgpu::DeviceType::IntegratedGpu => 1,
            wgpu::DeviceType::VirtualGpu => 2,
            _ => 3,
        });
        let adapter = adapters
            .into_iter()
            .find(|a| name == "auto" || a.get_info().name == name)
            .context("Selected GPU was not found")?;
        let info = adapter.get_info();
        let mut limits = wgpu::Limits::default();
        limits.max_storage_buffer_binding_size = adapter
            .limits()
            .max_storage_buffer_binding_size
            .min(160 * 1024 * 1024);
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("OpenCam processing"),
                required_limits: limits,
                ..Default::default()
            }))?;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                entry(0, true),
                entry(1, false),
                entry(2, false),
                entry(3, false),
                entry(4, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("OpenCam effects"),
            source: wgpu::ShaderSource::Wgsl(include_str!("effects.wgsl").into()),
        });
        let pipelines = ["horizontal", "vertical", "effects"].map(|entry| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        });
        let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&Params {
                dimensions: [0; 4],
                effects: [0.; 4],
                local: [0.; 4],
                aspect: [0.; 4],
                flags: [0; 4],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let mask = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Person mask"),
            size: 256 * 144 * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            name: format!("{} · {:?}", info.name, info.backend),
            device,
            queue,
            params,
            mask,
            layout,
            pipelines,
            buffers: None,
        })
    }
    pub fn process(&mut self, frame: &Frame, o: &Options, mask: Option<&[f32]>) -> Result<Frame> {
        let (w, h) = o.dimensions(frame.width, frame.height);
        let key = (frame.width, frame.height, w, h);
        let bytes = frame.pixels.len() as u64;
        let output_bytes = u64::from(w) * u64::from(h) * 4;
        ensure!(
            bytes <= u64::from(self.device.limits().max_storage_buffer_binding_size)
                && output_bytes <= u64::from(self.device.limits().max_storage_buffer_binding_size),
            "Frame exceeds GPU buffer limit"
        );
        if self.buffers.as_ref().is_none_or(|b| b.key != key) {
            let create = |size, usage| {
                self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size,
                    usage,
                    mapped_at_creation: false,
                })
            };
            let input = create(
                bytes,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            );
            let blur_a = create(bytes, wgpu::BufferUsages::STORAGE);
            let blur_b = create(bytes, wgpu::BufferUsages::STORAGE);
            let output = create(
                output_bytes,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            );
            let readback = create(
                output_bytes,
                wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            );
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.layout,
                entries: &[&input, &blur_a, &blur_b, &output, &self.mask, &self.params]
                    .into_iter()
                    .enumerate()
                    .map(|(binding, buffer)| wgpu::BindGroupEntry {
                        binding: binding as u32,
                        resource: buffer.as_entire_binding(),
                    })
                    .collect::<Vec<_>>(),
            });
            self.buffers = Some(Buffers {
                key,
                input,
                _blur_a: blur_a,
                _blur_b: blur_b,
                output,
                readback,
                group,
            });
        }
        let b = self.buffers.as_ref().unwrap();
        let scale = o.scale(frame.width, frame.height);
        self.queue.write_buffer(&b.input, 0, &frame.pixels);
        if let Some(mask) = mask {
            ensure!(mask.len() == 256 * 144, "Invalid mask dimensions");
            self.queue
                .write_buffer(&self.mask, 0, bytemuck::cast_slice(mask));
        }
        self.queue.write_buffer(
            &self.params,
            0,
            bytemuck::bytes_of(&Params {
                dimensions: [frame.width, frame.height, w, h],
                effects: [o.stretch[0], o.stretch[1], o.distortion, o.bulge],
                local: [o.center[0], o.center[1], o.radius, o.blur],
                aspect: [scale[0], scale[1], 0., 0.],
                flags: [
                    u32::from(o.rotation),
                    u32::from(o.mirror),
                    u32::from(mask.is_some()),
                    0,
                ],
            }),
        );
        let mut encoder = self.device.create_command_encoder(&Default::default());
        for pass in 0..3 {
            if pass < 2 && o.blur == 0. {
                continue;
            }
            let mut compute = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            compute.set_pipeline(&self.pipelines[pass]);
            compute.set_bind_group(0, &b.group, &[]);
            let (x, y) = if pass == 2 {
                (w, h)
            } else {
                (frame.width, frame.height)
            };
            compute.dispatch_workgroups(x.div_ceil(8), y.div_ceil(8), 1);
        }
        // ponytail: read back for Zui/virtual drivers until both accept shared GPU textures.
        encoder.copy_buffer_to_buffer(&b.output, 0, &b.readback, 0, output_bytes);
        let submission = self.queue.submit([encoder.finish()]);
        let (tx, rx) = mpsc::sync_channel(1);
        b.readback.map_async(wgpu::MapMode::Read, .., move |r| {
            let _ = tx.send(r);
        });
        if let Err(e) = self.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(2)),
        }) {
            b.readback.unmap();
            return Err(e.into());
        }
        rx.recv_timeout(Duration::from_secs(2))??;
        let pixels = b.readback.get_mapped_range(..).to_vec();
        b.readback.unmap();
        Ok(Frame {
            pixels,
            width: w,
            height: h,
            sequence: frame.sequence,
        })
    }
}
fn entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
