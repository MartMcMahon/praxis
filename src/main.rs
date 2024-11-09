use camera::Camera;
use camera::CameraUniform;
use cgmath::InnerSpace;
use cgmath::Zero;
use cube::{Cube, DrawModel};
use std::sync::Arc;
use timer::Timer;
use vertex::{BasicVertex, EffectVertex, Vertex};
use wgpu::util::DeviceExt;
use wgpu::Surface;
use wgpu_text::glyph_brush::ab_glyph::FontRef;
use wgpu_text::glyph_brush::{OwnedSection, Section as TextSection, Text};
use wgpu_text::TextBrush;
use winit::application::ApplicationHandler;
use winit::event::{KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

mod camera;
mod controller;
mod cube;
mod texture;
mod timer;
mod vertex;

struct Instance {
    position: cgmath::Vector3<f32>,
    rotation: cgmath::Quaternion<f32>,
}
impl Instance {
    fn to_raw(&self) -> InstanceRaw {
        InstanceRaw {
            model: (cgmath::Matrix4::from_translation(self.position)
                * cgmath::Matrix4::from(self.rotation))
            .into(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceRaw {
    model: [[f32; 4]; 4],
}
impl InstanceRaw {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

const EFFECT_VERTS: &[EffectVertex] = &[
    EffectVertex {
        position: [-1.0, 1.0, 0.0],
        color: [1.0, 0.0, 0.0],
    },
    EffectVertex {
        position: [1.0, 1.0, 0.0],
        color: [0.0, 1.0, 0.0],
    },
    EffectVertex {
        position: [1.0, -1.0, 0.0],
        color: [0.0, 0.0, 1.0],
    },
    EffectVertex {
        position: [-1.0, -1.0, 0.0],
        color: [0.4, 0.4, 0.1],
    },
];
const EFFECT_INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

const BACKGROUND_QUAD: &[BasicVertex] = &[
    BasicVertex {
        position: [-1.0, 1.0, 0.0],
        tex_coords: [0.0, 0.0],
    },
    BasicVertex {
        position: [1.0, 1.0, 0.0],
        tex_coords: [1.0, 0.0],
    },
    BasicVertex {
        position: [1.0, -1.0, 0.0],
        tex_coords: [1.0, 1.0],
    },
    BasicVertex {
        position: [-1.0, -1.0, 0.0],
        tex_coords: [0.0, 1.0],
    },
];
const BACKGROUND_QUAD_INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];
#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    instance: Option<wgpu::Instance>,
    surface: Option<Surface<'static>>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,

    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    timer: Option<Timer>,

    brush: Option<TextBrush<FontRef<'static>>>,
    text_section: Option<OwnedSection>,

    // camera
    camera: Option<Camera>,
    camera_buffer: Option<wgpu::Buffer>,
    camera_bind_group: Option<wgpu::BindGroup>,

    // background texture
    background_render_pipeline: Option<wgpu::RenderPipeline>,
    background_texture_bind_group: Option<wgpu::BindGroup>,
    background_vertex_buffer: Option<wgpu::Buffer>,
    background_index_buffer: Option<wgpu::Buffer>,

    // cube
    cube_pipeline: Option<wgpu::RenderPipeline>,
    cube_bind_group: Option<wgpu::BindGroup>,
    cube_vertex_buf: Option<wgpu::Buffer>,
    cube_index_buf: Option<wgpu::Buffer>,
    cube_instances: Vec<Instance>,
    cube_instance_buffer: Option<wgpu::Buffer>,
    cube_model: Option<cube::Cube>,

    // player
    cube_position: Option<cgmath::Vector3<f32>>,

    // controller
    controller: controller::Controller,
}

const WIDTH: u32 = 1024;
const HEIGHT: u32 = 768;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        ///// window
        self.window = Some(Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        // .with_decorations(false)
                        .with_inner_size(winit::dpi::LogicalSize::new(WIDTH, HEIGHT))
                        // .with_position(winit::dpi::LogicalPosition::new(x, y))
                        .with_transparent(true), // .with_window_level(WindowLevel::AlwaysOnTop),
                )
                .unwrap(),
        ));

        self.instance = Some(wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: wgpu::InstanceFlags::empty(),
            ..Default::default()
        }));
        self.surface = Some(
            self.instance
                .as_ref()
                .unwrap()
                .create_surface(self.window.clone().unwrap())
                .unwrap(),
        );

        let adapter = pollster::block_on(self.instance.as_ref().unwrap().request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: self.surface.as_ref(),
                force_fallback_adapter: false,
            },
        ))
        .unwrap();
        let device_queue = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("device-descriptor"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            },
            None,
        ))
        .unwrap();

        self.device = Some(device_queue.0);
        self.queue = Some(device_queue.1);

        let texture_format = wgpu::TextureFormat::Bgra8UnormSrgb;

        self.camera = Some(Camera {
            eye: (8.4, 25.0, -8.4).into(),
            target: (0.0, 0.0, 0.0).into(),
            up: (0.0, 1.0, 0.0).into(),
            aspect: WIDTH as f32 / HEIGHT as f32,
            fovy: 90.0,
            znear: 0.1,
            zfar: 100.0,
        });

        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&self.camera.as_ref().unwrap());

        self.camera_buffer = Some(self.device.as_ref().unwrap().create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[camera_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            },
        ));

        let cube_bind_group_layout = &self.device.as_ref().unwrap().create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("cube bind group layout"),
            },
        );

        let camera_bind_group_layout = self.device.as_ref().unwrap().create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
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
                label: Some("camera_bind_group_layout"),
            },
        );

        self.camera_bind_group = Some(self.device.as_ref().unwrap().create_bind_group(
            &wgpu::BindGroupDescriptor {
                layout: &camera_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_ref().unwrap().as_entire_binding(),
                }],
                label: Some("camera_bind_group"),
            },
        ));

        let size = self.window.as_ref().unwrap().inner_size();
        self.surface.as_ref().unwrap().configure(
            &self.device.as_ref().unwrap(),
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                // not really sure what the TextureFormat is
                format: texture_format,
                width: size.width,
                height: size.height,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 1,
                alpha_mode: wgpu::CompositeAlphaMode::PostMultiplied,
                // alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
            },
        );

        ////// controller
        self.controller.velocity = 0.5; // = controller::Controller::new(0.5);

        /////// brush stuff
        let font = include_bytes!("../res/fonts/Fira_Code_v6.2/ttf/FiraCode-Light.ttf") as &[u8];
        self.brush = Some(
            wgpu_text::BrushBuilder::using_font_bytes(font)
                .unwrap()
                .build(self.device.as_ref().unwrap(), WIDTH, HEIGHT, texture_format),
        );

        self.text_section = Some(
            TextSection::default()
                .add_text(Text::new("Hello!  はじめまして!").with_color([0.9, 1.0, 1.0, 1.0]))
                .with_bounds((WIDTH as f32, HEIGHT as f32))
                .with_layout(
                    wgpu_text::glyph_brush::Layout::default()
                        .v_align(wgpu_text::glyph_brush::VerticalAlign::Center),
                )
                // .with_screen_position((0.0, 0.0))
                .to_owned(),
        );
        ////

        //// uniform buffer
        self.timer = Some(Timer::new(self.device.as_ref().unwrap()));

        self.vertex_buffer = Some(self.device.as_ref().unwrap().create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(EFFECT_VERTS),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
        // index buffer
        self.index_buffer = Some(self.device.as_ref().unwrap().create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(EFFECT_INDICES),
                usage: wgpu::BufferUsages::INDEX,
            },
        ));

        // camera stuff
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&self.camera.as_ref().unwrap());

        let cube_shader =
            self.device
                .as_ref()
                .unwrap()
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("Shader"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("cube.wgsl").into()),
                });

        let camera_bind_group_layout = &self.device.as_ref().unwrap().create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
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
                label: Some("camera_bind_group_layout"),
            },
        );

        let cube_render_pipeline_layout =
            self.device
                .as_ref()
                .unwrap()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("cube pipeline layout"),
                    bind_group_layouts: &[cube_bind_group_layout, &camera_bind_group_layout],
                    push_constant_ranges: &[],
                });

        ///// shader time
        let basic_shader =
            self.device
                .as_ref()
                .unwrap()
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("Shader"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("basic.wgsl").into()),
                });
        let background_texture_bind_group_layout =
            &self.device.as_ref().unwrap().create_bind_group_layout(
                &wgpu::BindGroupLayoutDescriptor {
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                    label: Some("background texture bind group layout"),
                },
            );
        let background_render_pipeline_layout = self
            .device
            .as_ref()
            .unwrap()
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("cube pipeline layout"),
                bind_group_layouts: &[background_texture_bind_group_layout],
                push_constant_ranges: &[],
            });
        self.background_render_pipeline =
            Some(self.device.as_ref().unwrap().create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("background render pipeline"),
                    layout: Some(&background_render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &basic_shader,
                        entry_point: "vs_main",
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[BasicVertex::desc()],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &basic_shader,
                        entry_point: "fs_main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: texture_format,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Cw,
                        cull_mode: Some(wgpu::Face::Back),
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                    cache: None,
                },
            ));
        self.background_vertex_buffer = Some(self.device.as_ref().unwrap().create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("background vertex buffer"),
                contents: bytemuck::cast_slice(BACKGROUND_QUAD.to_vec().as_slice()),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
        self.background_index_buffer = Some(self.device.as_ref().unwrap().create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("background index buffer"),
                contents: bytemuck::cast_slice(BACKGROUND_QUAD_INDICES),
                usage: wgpu::BufferUsages::INDEX,
            },
        ));

        let background_diffuse_bytes = include_bytes!("../res/backgrounds/reactor.png");
        let background_diffuse_texture = texture::Texture::from_bytes(
            &self.device.as_ref().unwrap(),
            &self.queue.as_ref().unwrap(),
            background_diffuse_bytes,
            "background image",
            false,
        )
        .unwrap();
        self.background_texture_bind_group = Some(self.device.as_ref().unwrap().create_bind_group(
            &wgpu::BindGroupDescriptor {
                layout: &background_texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &background_diffuse_texture.view,
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(
                            &background_diffuse_texture.sampler,
                        ),
                    },
                ],
                label: Some("backgroundd texture bind group"),
            },
        ));

        self.cube_pipeline = Some(self.device.as_ref().unwrap().create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("cube render pipeline"),
                layout: Some(&cube_render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &cube_shader,
                    entry_point: "vs_main",
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[vertex::ModelVertex::desc(), InstanceRaw::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &basic_shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: texture_format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            },
        ));

        self.cube_model = Some(
            cube::load_cube(
                "cube.obj",
                &self.device.as_ref().unwrap(),
                &self.queue.as_ref().unwrap(),
                cube_bind_group_layout,
            )
            .unwrap(),
        );

        self.cube_position = Some(cgmath::Vector3 {
            x: -1.0,
            y: -1.0,
            z: -1.0,
        });

        use cgmath::prelude::*;
        const SPACE_BETWEEN: f32 = 3.0;
        const NUM_INSTANCES_PER_ROW: i32 = 5;

        self.cube_instances = vec![Instance {
            position: self.cube_position.unwrap(),
            rotation: cgmath::Quaternion::zero(),
            // cgmath::Quaternion::from_axis_angle(
            //     (16.6,50.0,-16.6).into(),
            // cgmath::Deg(45.0)
            // ),
        }];

        // self.cube_instances = (0..NUM_INSTANCES_PER_ROW)
        //     .flat_map(|y| {
        //         (0..NUM_INSTANCES_PER_ROW).map(move |x| {
        //             let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
        //             let y = SPACE_BETWEEN * (y as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);

        //             let mut position = cgmath::Vector3 { x, y, z: 0.1 };
        //             // position.x = self.cube_position.as_ref().unwrap().clone().x;
        //             // position.y = self.cube_position[1].into();
        //             // position.z = self.cube_position[2].into();
        //             // let position = &self.cube_position.unwrap();

        //             let rotation = if position.is_zero() {
        //                 cgmath::Quaternion::from_axis_angle(
        //                     cgmath::Vector3::unit_z(),
        //                     cgmath::Deg(0.0),
        //                 )
        //             } else {
        //                 cgmath::Quaternion::from_axis_angle(position.normalize(), cgmath::Deg(45.0))
        //             };

        //             Instance { position, rotation }
        //         })
        //     })
        //     .collect::<Vec<_>>();

        let instance_data = self
            .cube_instances
            .iter()
            .map(Instance::to_raw)
            .collect::<Vec<_>>();

        self.cube_instance_buffer = Some(self.device.as_ref().unwrap().create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("cube instance buffer"),
                contents: bytemuck::cast_slice(&instance_data),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            },
        ));

        //////
        // in new() after creating `camera`
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&self.camera.as_ref().unwrap());

        // initial redraw request
        self.window.as_ref().unwrap().request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.controller.process_events(&event) {
            return;
        }
        match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: winit::event::ElementState::Pressed,
                        logical_key: Key::Named(NamedKey::Escape),
                        ..
                    },
                ..
            } => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: winit::event::ElementState::Pressed,
                        logical_key: Key::Named(NamedKey::Space),
                        ..
                    },
                ..
            } => self.add_cube(),

            WindowEvent::RedrawRequested => {
                self.update();
                let output = self
                    .surface
                    .as_ref()
                    .unwrap()
                    .get_current_texture()
                    .unwrap();

                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = self.device.as_ref().unwrap().create_command_encoder(
                    &wgpu::CommandEncoderDescriptor {
                        label: Some("render encoder"),
                    },
                );

                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("render pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.1,
                                    g: 0.2,
                                    b: 0.3,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    //////
                    // draw background
                    render_pass.set_pipeline(&self.background_render_pipeline.as_ref().unwrap());
                    render_pass.set_bind_group(
                        0,
                        &self.background_texture_bind_group.as_ref().unwrap(),
                        &[],
                    );
                    render_pass.set_bind_group(1, self.camera_bind_group.as_ref().unwrap(), &[]);
                    render_pass.set_vertex_buffer(
                        0,
                        self.background_vertex_buffer.as_ref().unwrap().slice(..),
                    );
                    render_pass.set_index_buffer(
                        self.background_index_buffer.as_ref().unwrap().slice(..),
                        wgpu::IndexFormat::Uint16,
                    );
                    // render_pass.draw_indexed(0..BACKGROUND_QUAD_INDICES.len() as u32, 0, 0..1);

                    ///////
                    // cube
                    render_pass.set_pipeline(&self.cube_pipeline.as_ref().unwrap());
                    // render_pass.set_vertex_buffer(
                    //     0,
                    //     self.cube_model.as_ref().unwrap().meshes[0]
                    //         .vertex_buffer
                    //         .slice(..),
                    // );
                    // let material = &self.cube_model.as_ref().unwrap().materials[0].bind_group;
                    // render_pass.set_bind_group(0, &material, &[]);
                    // render_pass.set_index_buffer(
                    //     self.cube_model.as_ref().unwrap().meshes[0]
                    //         .index_buffer
                    //         .slice(..),
                    //     wgpu::IndexFormat::Uint16,
                    // );
                    // render_pass.draw_indexed(0..8, 0, 0..1);
                    // /////////////
                    render_pass.set_vertex_buffer(
                        1,
                        self.cube_instance_buffer.as_ref().unwrap().slice(..),
                    );
                    let mesh = &self.cube_model.as_ref().unwrap().meshes[0];
                    let material = &self.cube_model.as_ref().unwrap().materials[0];
                    render_pass.set_bind_group(0, &material.bind_group, &[]);
                    render_pass.draw_mesh_instanced(
                        mesh,
                        material,
                        0..self.cube_instances.len() as u32,
                        self.camera_bind_group.as_ref().unwrap(),
                    );

                    self.brush.as_ref().unwrap().draw(&mut render_pass);
                }

                // submit will accept anything that implements IntoIter
                self.queue
                    .as_ref()
                    .unwrap()
                    .submit(std::iter::once(encoder.finish()));
                output.present();
                self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
    }
}
impl App {
    fn update(&mut self) {
        // Update the cube's position
        let mut x = 0.0;
        let mut y = 0.0;
        let mut z = 0.0;
        if self.controller.is_up_pressed {
            z += 1.0;
        }
        if self.controller.is_down_pressed {
            z -= 1.0;
        }
        if self.controller.is_left_pressed {
            x -= 1.0;
        }
        if self.controller.is_right_pressed {
            x += 1.0;
        }
        let mut move_vector = cgmath::Vector3::new(x, y, z);
        if move_vector.magnitude() != 0.0 {
            move_vector = move_vector.normalize();
        }
        move_vector *= self.controller.velocity;

        for c in self.cube_instances.iter_mut() {
            c.position += move_vector;
        }
        // self.cube_instances[0].position += move_vector;

        // Map the instance data to `InstanceRaw` format
        let instance_data = self
            .cube_instances
            .iter()
            .map(Instance::to_raw)
            .collect::<Vec<_>>();

        // Re-upload the updated instance data to the GPU
        self.queue.as_ref().unwrap().write_buffer(
            self.cube_instance_buffer.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(&instance_data),
        );

        match self.timer.as_mut() {
            Some(timer) => {
                let target_fps = 1.0 / 60.0 as f64;
                timer.elapsed = timer.start.elapsed().as_secs_f64();
                timer.acc += timer.elapsed - timer.last;
                timer.last = timer.elapsed;
                // framerate stuff goes here?
                timer.timer_uniform.t = timer.elapsed as f32;
                self.queue.as_ref().unwrap().write_buffer(
                    &timer.timer_buffer,
                    0,
                    &timer.timer_uniform.t.to_le_bytes(),
                );
            }
            None => {}
        };
    }

    fn add_cube(&mut self) {
        let x: f32 = rand::random::<f32>() * 10.0;
        let y: f32 = rand::random::<f32>() * 10.0;
        let z: f32 = rand::random::<f32>() * 10.0;
        let position = (x, y, z).into();

        self.cube_instances.push(Instance {
            position,
            rotation: cgmath::Quaternion::zero(),
        });

        let instance_data = self
            .cube_instances
            .iter()
            .map(Instance::to_raw)
            .collect::<Vec<_>>();

        self.cube_instance_buffer = Some(self.device.as_ref().unwrap().create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("cube instance buffer"),
                contents: bytemuck::cast_slice(&instance_data),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            },
        ));

        // // Map the instance data to `InstanceRaw` format
        // let instance_data = self
        //     .cube_instances
        //     .iter()
        //     .map(Instance::to_raw)
        //     .collect::<Vec<_>>();

        // // Re-upload the updated instance data to the GPU
        self.queue.as_ref().unwrap().write_buffer(
            self.cube_instance_buffer.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(&instance_data),
        );
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::default();
    let _ = event_loop.run_app(&mut app);
}
