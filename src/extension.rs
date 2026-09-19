use mod_api_stable::*;

pub struct VfxExtension;

impl StableExtension for VfxExtension {
    fn post_render(&self, client: &mut StableClient<'_>) {
        if !client.is_in_game() || !client.can_draw() {
            return;
        }
        for snapshot in crate::state::buff_vfx_snapshot() {
            let Some(binding) = crate::buff_visuals::binding(&snapshot.binding_name) else {
                continue;
            };
            let Some(frame) = crate::buff_visuals::frame_for(binding, snapshot.elapsed_ticks) else {
                continue;
            };
            let params = StableSpriteParams {
                x: snapshot.x,
                y: snapshot.y,
                z: binding.z,
                pivot_x: 0.5,
                pivot_y: 0.5,
                uv: frame.uv,
                sample_nearest: true,
                ..StableSpriteParams::default()
            };
            let _ = client.draw_sprite("Game", binding.texture, &params);
        }
    }
}
