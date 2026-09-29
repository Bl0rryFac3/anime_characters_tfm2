use mod_api_stable::*;

const EVOLVE_COUNTER: &str = "kirito_evolve";
const EVOLVE_1_BUFF: &str = "kirito_evolve_1";
const EVOLVE_2_BUFF: &str = "kirito_evolve_2";

#[derive(Debug)]
pub struct KiritoEvolveTick;

impl StableEffectType for KiritoEvolveTick {
    fn apply(&self, sim: &mut StableSim<'_>, _rng_seed: u64, caster_id: usize, _input: InputTargetV1) {
        crate::state::add_counter(sim, caster_id, EVOLVE_COUNTER, 1);
        let total = crate::state::counter_value(sim, caster_id, EVOLVE_COUNTER);

        if total >= 100 && !crate::helpers::has_buff(sim, caster_id, EVOLVE_1_BUFF) {
            let mut buff = BuffV1::named(EVOLVE_1_BUFF);
            buff.attack = 10;
            buff.defence = 5;
            crate::state::add_buff(sim, caster_id, &buff, true, false, 1, true);
            sim.play_view_effect("kirito_circular", caster_id, &InputTargetV1::target(caster_id), 0, 0, 60);
        }

        if total >= 200 && !crate::helpers::has_buff(sim, caster_id, EVOLVE_2_BUFF) {
            let mut buff = BuffV1::named(EVOLVE_2_BUFF);
            buff.attack = 20;
            buff.defence = 10;
            buff.move_speed_mult = 20;
            crate::state::add_buff(sim, caster_id, &buff, true, false, 1, true);
            sim.play_view_effect("kirito_starburst", caster_id, &InputTargetV1::target(caster_id), 0, 0, 60);
        }
    }
}
