// --- Add to a new file, e.g. src/paragon.rs, or into native_effects/mod.rs
// alongside everything else.

use mod_api_stable::*;

const ASCENSION_BUFF: &str = "paragon_ascension";
const MAX_ASCENSION_STACKS: usize = 30;

fn ascension_stack() -> BuffV1 {
    let mut b = BuffV1::named(ASCENSION_BUFF); // permanent — stacks persist for the fight
    b.attack_mult = 5; // +5% per stack, placeholder — tune once he's testable
    b
}

#[derive(Debug, Clone, Default)]
pub struct ParagonPassive {
    tick_accumulator: u32,
}

impl StablePassive for ParagonPassive {
    fn clone_box(&self) -> Box<dyn StablePassive> {
        Box::new(self.clone())
    }

    fn on_update(&mut self, sim: &mut StableSim<'_>, _rng_seed: u64, _player: usize, entity: usize) {
        self.tick_accumulator += 1;
        if self.tick_accumulator < 60 { return; } // once per assumed-second, throttled
        self.tick_accumulator = 0;

        let Some((team, (cx, cy))) = sim.get_entity(entity).map(|e| (e.team(), e.pos())) else { return };
        let range_sq: u64 = 15000 * 15000; // "near an enemy" radius, placeholder

        let mut near_enemy = false;
        let count = sim.entity_count();
        for i in 0..count {
            if let Some(other) = sim.entity_at(i) {
                if other.is_alive() && other.team() != team {
                    let (ex, ey) = other.pos();
                    let dx = ex as i64 - cx as i64;
                    let dy = ey as i64 - cy as i64;
                    if (dx * dx + dy * dy) as u64 <= range_sq {
                        near_enemy = true;
                        break;
                    }
                }
            }
        }

        if near_enemy {
            sim.entity_stack_buff(entity, &ascension_stack(), MAX_ASCENSION_STACKS, false);
        }
    }

    fn on_kill(&mut self, sim: &mut StableSim<'_>, _player: usize, entity: usize, _victim: usize) {
        // Takedown burst: five stacks at once, rewarding real kills over passive farming.
        for _ in 0..5 {
            sim.entity_stack_buff(entity, &ascension_stack(), MAX_ASCENSION_STACKS, false);
        }
    }
}
