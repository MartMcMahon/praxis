use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
// bytemuck::Pod, bytemuck::Zeroable)]
pub struct TimerUniform {
    pub t: f32,
}
#[repr(C)]
pub struct Timer {
    pub start: std::time::Instant,
    pub elapsed: f64,
    pub last: f64,
    pub acc: f64,
    pub timer_uniform: TimerUniform,
    pub timer_buffer: wgpu::Buffer,
    pub timer_bind_group: wgpu::BindGroup,
    pub timer_bind_group_layout: wgpu::BindGroupLayout,
}
impl Timer {
    pub fn new(device: &wgpu::Device) -> Self {
        let timer_uniform = TimerUniform { t: 0.2 };
        let timer_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Timer Buffer"),
            contents: &timer_uniform.t.to_le_bytes(),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let timer_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bind_group_for_timer_uniform"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },

                    count: None,
                }],
            });

        let timer_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &timer_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: timer_buffer.as_entire_binding(),
            }],
        });

        let start = std::time::Instant::now();

        Timer {
            start,
            elapsed: 0.0,
            last: 0.0,
            acc: 0.0f64,
            timer_uniform,
            timer_buffer,
            timer_bind_group,
            timer_bind_group_layout,
        }
    }
}
