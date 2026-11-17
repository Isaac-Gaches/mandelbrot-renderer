use std::default::Default;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder,Window},
};
use wgpu::util::DeviceExt;
use encase::ShaderType;
use wgpu::core::device::RenderPassCompatibilityCheckType::RenderPipeline;
use wgpu::core::instance::RequestAdapterOptions;
use wgpu::{InstanceDescriptor, RenderPassColorAttachment};

const SCALE_SPEED: f32 = 0.02;
const CAMERA_SPEED: f32 = 0.08;
#[derive(Debug,ShaderType)]
struct State{
    camera_position: glam::Vec2,
    scale: f32,
    max_iterations: u32,
}
impl Default for State{
    fn default() -> Self {
        Self{
            camera_position: glam::Vec2::ZERO,
            scale: 1.0,
            max_iterations: 200,
        }
    }
}
impl State{
    fn as_wgsl_bytes(&self) -> encase::internal::Result<Vec<u8>> {
        let mut buffer = encase::UniformBuffer::new(Vec::new());
        buffer.write(self)?;
        Ok(buffer.into_inner())
    }
    fn translate_view(&mut self, increments: i32, axis: usize) {
        self.camera_position[axis] += CAMERA_SPEED * increments as f32 / self.scale;
    }
    fn zoom(&mut self, amount: f32) {
        self.scale += SCALE_SPEED * amount * self.scale.powf(1.02);
        self.scale = self.scale.max(1.1);
    }
}
struct Context{
    device: wgpu::Device,
    surface: wgpu::Surface,
    surface_config: wgpu::SurfaceConfiguration,
    window: Window,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
}
impl Context{
    async fn new(window:Window)->Self{
        let size = window.inner_size();
        let instance = wgpu::Instance::new(InstanceDescriptor{
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = unsafe{instance.create_surface(&window)}.unwrap();
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions{
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }).await.unwrap();
        let (device,queue) = adapter.request_device(&wgpu::DeviceDescriptor{
            label: None,
            features: wgpu::Features::empty(),
            limits: Default::default(), }, None
        ).await.unwrap();
        let surface_capabilities = surface.get_capabilities(&adapter);
        let surface_format = surface_capabilities.formats[0];
        let surface_config = wgpu::SurfaceConfiguration{
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_capabilities.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device,&surface_config);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor{
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor{
            label: None,
            size: std::mem::size_of::<State>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor{
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry{
                binding: 0,
                visibility: wgpu::ShaderStages::all(),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor{
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry{
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding{
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });
        let render_pipeline_layout= device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor{
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
            label: None,
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState{
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState{
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(surface_format.into())],
            }),
            multiview: None,
        });
        Self{
            surface,
            device,
            surface_config,
            queue,
            bind_group,
            uniform_buffer,
            pipeline,
            window,
        }
    }
    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.surface_config.width = new_size.width;
        self.surface_config.height = new_size.height;
        self.surface.configure(&self.device, &self.surface_config);
        self.window.request_redraw();
    }
    fn render(&mut self,state:&State){
        let frame = self.surface.get_current_texture().unwrap();
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.queue.write_buffer(&self.uniform_buffer, 0, &state.as_wgsl_bytes().unwrap());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor{
            label: None,
        });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor{
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment{
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations{
                        load:wgpu::LoadOp::Clear(wgpu::Color{
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0,&self.bind_group,&[]);
            render_pass.draw(0..3,0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}
async fn run(){
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&*event_loop).unwrap();
    let mut context = Context::new(window).await;
    let mut state = State::default();
    event_loop.run(move |event,_,control_flow|{
        match event {
            Event::WindowEvent {ref event, window_id} if window_id == context.window.id()=>{
                match event {
                    WindowEvent::CloseRequested=>*control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(physical_size)=>context.resize(*physical_size),
                    WindowEvent::ScaleFactorChanged{new_inner_size,..}=>context.resize(**new_inner_size),
                    WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            state:ElementState::Pressed,
                            virtual_keycode: Some(keycode),
                            ..
                        },
                        ..
                    } => {
                        match keycode {
                            VirtualKeyCode::W => {
                                state.translate_view(1,1);
                            }
                            VirtualKeyCode::A => {
                                state.translate_view(-1,0);
                            }
                            VirtualKeyCode::S => {
                                state.translate_view(-1,1);
                            }
                            VirtualKeyCode::D => {
                                state.translate_view(1,0);
                            }
                            VirtualKeyCode::Up => {
                                state.zoom(1.0);
                            }
                            VirtualKeyCode::Down => {
                                state.zoom(-1.0);
                            }
                            _ => {},
                        }
                    }
                    _=>{}
                }
            }
            Event::RedrawRequested(window_id) if window_id == context.window.id()=>{
                context.render(&state);
            }
            Event::MainEventsCleared=>context.window.request_redraw(),
            _=>{}
        }
    });
}
fn main() {
    pollster::block_on(run());
}

/*use std::default::Default;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder,Window},
};
use wgpu::util::DeviceExt;
use encase::ShaderType;
use wgpu::core::device::RenderPassCompatibilityCheckType::RenderPipeline;
use wgpu::core::instance::RequestAdapterOptions;
use wgpu::{InstanceDescriptor, RenderPassColorAttachment};

const SCALE_SPEED: f32 = 0.02;
const CAMERA_SPEED: f32 = 0.08;
#[derive(Debug,ShaderType)]
struct State{
    camera_position: glam::Vec2,
    scale: f32,
    max_iterations: u32,
}
impl Default for State{
    fn default() -> Self {
        Self{
            camera_position: glam::Vec2::ZERO,
            scale: 1.0,
            max_iterations: 200,
        }
    }
}
impl State{
    fn as_wgsl_bytes(&self) -> encase::internal::Result<Vec<u8>> {
        let mut buffer = encase::UniformBuffer::new(Vec::new());
        buffer.write(self)?;
        Ok(buffer.into_inner())
    }
    fn translate_view(&mut self, increments: i32, axis: usize) {
        self.camera_position[axis] += CAMERA_SPEED * increments as f32 / self.scale;
    }
    fn zoom(&mut self, amount: f32) {
        self.scale += SCALE_SPEED * amount * self.scale.powf(1.02);
        self.scale = self.scale.max(1.1);
    }
}
struct Context{
    device: wgpu::Device,
    surface: wgpu::Surface,
    surface_config: wgpu::SurfaceConfiguration,
    window: Window,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
}
impl Context{
    async fn new(window:Window)->Self{
        let size = window.inner_size();
        let instance = wgpu::Instance::new(InstanceDescriptor{
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = unsafe{instance.create_surface(&window)}.unwrap();
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions{
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }).await.unwrap();
        let (device,queue) = adapter.request_device(&wgpu::DeviceDescriptor{
            label: None,
            features: wgpu::Features::empty(),
            limits: Default::default(), }, None
        ).await.unwrap();
        let surface_capabilities = surface.get_capabilities(&adapter);
        let surface_format = surface_capabilities.formats[0];
        let surface_config = wgpu::SurfaceConfiguration{
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_capabilities.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device,&surface_config);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor{
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor{
            label: None,
            size: std::mem::size_of::<State>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor{
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry{
                binding: 0,
                visibility: wgpu::ShaderStages::all(),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor{
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry{
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding{
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });
        let render_pipeline_layout= device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor{
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
            label: None,
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState{
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState{
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(surface_format.into())],
            }),
            multiview: None,
        });
        Self{
            surface,
            device,
            surface_config,
            queue,
            bind_group,
            uniform_buffer,
            pipeline,
            window,
        }
    }
    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.surface_config.width = new_size.width;
        self.surface_config.height = new_size.height;
        self.surface.configure(&self.device, &self.surface_config);
        self.window.request_redraw();
    }
    fn render(&mut self,state:&State){
        let frame = self.surface.get_current_texture().unwrap();
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.queue.write_buffer(&self.uniform_buffer, 0, &state.as_wgsl_bytes().unwrap());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor{
            label: None,
        });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor{
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment{
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations{
                        load:wgpu::LoadOp::Clear(wgpu::Color{
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0,&self.bind_group,&[]);
            render_pass.draw(0..3,0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}
async fn run(){
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&*event_loop).unwrap();
    let mut context = Context::new(window).await;
    let mut state = State::default();
    event_loop.run(move |event,_,control_flow|{
        match event {
            Event::WindowEvent {ref event, window_id} if window_id == context.window.id()=>{
                match event {
                    WindowEvent::CloseRequested=>*control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(physical_size)=>context.resize(*physical_size),
                    WindowEvent::ScaleFactorChanged{new_inner_size,..}=>context.resize(**new_inner_size),
                    WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            state:ElementState::Pressed,
                            virtual_keycode: Some(keycode),
                            ..
                        },
                        ..
                    } => {
                        match keycode {
                            VirtualKeyCode::W => {
                                state.translate_view(1,1);
                            }
                            VirtualKeyCode::A => {
                                state.translate_view(-1,0);
                            }
                            VirtualKeyCode::S => {
                                state.translate_view(-1,1);
                            }
                            VirtualKeyCode::D => {
                                state.translate_view(1,0);
                            }
                            VirtualKeyCode::Up => {
                                state.zoom(1.0);
                            }
                            VirtualKeyCode::Down => {
                                state.zoom(-1.0);
                            }
                            _ => {},
                        }
                    }
                    _=>{}
                }
            }
            Event::RedrawRequested(window_id) if window_id == context.window.id()=>{
                context.render(&state);
            }
            Event::MainEventsCleared=>context.window.request_redraw(),
            _=>{}
        }
    });
}
fn main() {
    pollster::block_on(run());
}
*/
