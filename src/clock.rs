#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ClockBuffer {
    ms: f32,
}
impl ClockBuffer {
    pub fn new() -> ClockBuffer {
        ClockBuffer { ms: 0.0 }
    }
    pub fn update(&mut self, delta: f32) {
        self.ms += delta;
    }
}
