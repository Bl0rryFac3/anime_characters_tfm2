use mod_api_stable::*;

pub struct DriverHook {
    // casts: Mutex<HashMap<usize, CastState>>,
}

impl DriverHook {
    pub fn new() -> Self {
        Self {}
    }
}

impl StableMatchHook for DriverHook {
    fn on_match_start(&self, sim: &mut StableSim<'_>) {
        // Runtime state is process-global inside the DLL, so clear only the
        // namespace belonging to this concrete SimCtxV1.state instance.
        crate::state::clear_sim(sim);
        crate::state::clear_projectile_view_effects(sim);
    }

    fn on_match_tick(&self, sim: &mut StableSim<'_>, _rng_seed: u64) {
        for index in 0..sim.player_count() {
            let Some(player) = sim.player_at(index) else { continue; };
            let player_id = player.id();
            let alive = player.is_alive();
            let entity_id = player.champion().map(|champion| champion.id());
            if let Some(entity_id) = entity_id {
                crate::state::bind_entity(sim, player_id, entity_id);
            }
            crate::state::update_player_lifecycle(sim, player_id, entity_id, alive);
        }
        crate::state::refresh_buff_visuals(sim);
        crate::state::update_projectile_view_effects(sim);

        // --- Paragon: apply any active periodic DoT channels (Godless ult) ---
        let tick = sim.tick() as u64;
        let count = sim.entity_count();
        for i in 0..count {
            let Some(source_id) = sim.entity_at(i).map(|e| e.id()) else { continue };
            let due = crate::state::due_periodics(sim, source_id, tick);
            for instance in due {
                // Placeholder flat amount — real formula gets wired in once
                // Godless's own action block is written.
                sim.deal_damage_typed(instance.source_id, instance.target_id, 40, DamageTypeV1::Fixed, AttackTypeV1::Skill);
                crate::state::advance_periodic(sim, instance.source_id, instance.target_id, instance.effect_id);
            }
        }
    }

    fn check_match_end(&self, _sim: &mut StableSim<'_>) -> Option<bool> { None }
}
