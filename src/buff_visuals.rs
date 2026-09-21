use mod_api_stable::*;

#[derive(Clone, Copy)]
pub struct Frame {
    pub duration_ticks: usize,
    pub uv: (f32, f32, f32, f32),
}

pub struct Binding {
    pub name: &'static str,
    pub texture: &'static str,
    pub z: i32,
    pub frames: &'static [Frame],
}


static BINDINGS: &[Binding] = &[
];

pub fn binding(name: &str) -> Option<&'static Binding> {
    BINDINGS.iter().find(|binding| binding.name == name)
}

pub fn binding_exists(name: &str) -> bool {
    binding(name).is_some()
}

pub fn frame_for(binding: &Binding, elapsed_ticks: usize) -> Option<Frame> {
    if binding.frames.is_empty() {
        return None;
    }
    let total = binding.frames.iter().map(|frame| frame.duration_ticks.max(1)).sum::<usize>().max(1);
    let mut cursor = elapsed_ticks % total;
    for frame in binding.frames {
        let duration = frame.duration_ticks.max(1);
        if cursor < duration {
            return Some(*frame);
        }
        cursor = cursor.saturating_sub(duration);
    }
    binding.frames.last().copied()
}
