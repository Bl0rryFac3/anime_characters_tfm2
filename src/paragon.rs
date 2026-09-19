// --- Paragon: StableChampion skeleton. No .data_champion file exists for
// this champion — everything here IS his data. Add `mod paragon;` to
// lib.rs, and `reg.add_champion(Paragon);` inside `init()`.

use mod_api_stable::*;

pub struct Paragon;

impl StableChampion for Paragon {
    fn id(&self) -> String {
        "paragon".to_string()
    }

    fn name(&self) -> String {
        "Paragon".to_string()
    }

    fn category(&self) -> ChampionCategoryV1 {
        ChampionCategoryV1::Melee
    }

    fn tags(&self) -> Vec<ChampionTagV1> {
        vec![ChampionTagV1::Ad, ChampionTagV1::Melee, ChampionTagV1::Tank, ChampionTagV1::Cc]
    }

    fn stat(&self) -> StatV1 {
        StatV1 {
            attack: 90,
            magic_power: 0,
            hp: 1300, // deliberately tanky — fits the "immovable" identity
            defence: 35,
            magic_resistance: 30,
            move_speed: 1000, // slower than most — sells the "heavy, deliberate" movement
            hp_regen: 2,
            stack: 0,
            crit_chance: 0,
        }
    }

    fn growth(&self) -> StatV1 {
        StatV1 {
            attack: 18,
            magic_power: 0,
            hp: 130,
            defence: 6,
            magic_resistance: 5,
            move_speed: 5,
            hp_regen: 1,
            stack: 0,
            crit_chance: 0,
        }
    }

    fn attack(&self) -> Box<dyn StableAction> {
        Box::new(ParagonBasicAttack)
    }

    fn skill(&self) -> Box<dyn StableAction> {
        Box::new(ParagonWrathgrasp)
    }

    fn skill2(&self) -> Box<dyn StableAction> {
        Box::new(ParagonShatterChain)
    }

    fn ult(&self) -> Option<Box<dyn StableAction>> {
        Some(Box::new(ParagonGodless))
    }

    fn passive(&self) -> Option<Box<dyn StablePassive>> {
        Some(Box::new(crate::paragon_passive::ParagonPassive::default()))
    }
}

// --- Action stubs. Each needs a real StableEffectType behind it — these
// return placeholder durations/cooldowns for now so the whole thing
// compiles; the actual grab/fear-bind/godless logic gets written next.

#[derive(Clone)]
struct ParagonBasicAttack;
impl StableAction for ParagonBasicAttack {
    fn clone_box(&self) -> Box<dyn StableAction> { Box::new(self.clone()) }
    fn action_name(&self) -> String { "attack".to_string() }
    fn duration(&self) -> usize { 20 }
    fn cooltime(&self, _caster_stat: &StatV1, _caster_level: usize) -> usize { 50 }
    fn casting_target(&self) -> CastingTargetV1 { CastingTargetV1::Enemy }
    fn effect(&self) -> Option<StableEffectSpec> {
        Some(StableEffectSpec {
            range: 15000,
            growth_range: 0,
            start_timing: 6,
            casting: CastingTypeV1::Targeting,
            target: CastingTargetV1::Enemy,
            attack_type: AttackTypeV1::BaseAttack,
            effect: Box::new(ParagonBasicAttackEffect),
        })
    }
}

#[derive(Clone)]
struct ParagonWrathgrasp;
impl StableAction for ParagonWrathgrasp {
    fn clone_box(&self) -> Box<dyn StableAction> { Box::new(self.clone()) }
    fn action_name(&self) -> String { "skill".to_string() }
    fn duration(&self) -> usize { 24 }
    fn cooltime(&self, _caster_stat: &StatV1, _caster_level: usize) -> usize { 160 }
    fn casting_target(&self) -> CastingTargetV1 { CastingTargetV1::EnemyChampion }
    fn effect(&self) -> Option<StableEffectSpec> {
        Some(StableEffectSpec {
            range: 15000,
            growth_range: 0,
            start_timing: 5,
            casting: CastingTypeV1::Targeting,
            target: CastingTargetV1::EnemyChampion,
            attack_type: AttackTypeV1::Skill,
            effect: Box::new(ParagonWrathgraspEffect),
        })
    }
}

#[derive(Clone)]
struct ParagonShatterChain;
impl StableAction for ParagonShatterChain {
    fn clone_box(&self) -> Box<dyn StableAction> { Box::new(self.clone()) }
    fn action_name(&self) -> String { "skill2".to_string() }
    fn duration(&self) -> usize { 30 }
    fn cooltime(&self, _caster_stat: &StatV1, _caster_level: usize) -> usize { 200 }
    fn casting_target(&self) -> CastingTargetV1 { CastingTargetV1::EnemyChampion }
    fn effect(&self) -> Option<StableEffectSpec> {
        Some(StableEffectSpec {
            range: 15000,
            growth_range: 0,
            start_timing: 6,
            casting: CastingTypeV1::Targeting,
            target: CastingTargetV1::EnemyChampion,
            attack_type: AttackTypeV1::Skill,
            effect: Box::new(ParagonShatterChainEffect),
        })
    }
}

#[derive(Clone)]
struct ParagonGodless;
impl StableAction for ParagonGodless {
    fn clone_box(&self) -> Box<dyn StableAction> { Box::new(self.clone()) }
    fn action_name(&self) -> String { "ult".to_string() }
    fn duration(&self) -> usize { 40 }
    fn cooltime(&self, _caster_stat: &StatV1, _caster_level: usize) -> usize { 1200 }
    fn casting_target(&self) -> CastingTargetV1 { CastingTargetV1::None }
    fn effect(&self) -> Option<StableEffectSpec> {
        Some(StableEffectSpec {
            range: 0,
            growth_range: 0,
            start_timing: 8,
            casting: CastingTypeV1::None,
            target: CastingTargetV1::Enemy,
            attack_type: AttackTypeV1::Skill,
            effect: Box::new(ParagonGodlessEffect),
        })
    }
}

// ============================================================
// Real effect implementations behind each action above.
// ============================================================

#[derive(Debug)]
struct ParagonBasicAttackEffect;
impl StableEffectType for ParagonBasicAttackEffect {
    fn apply(&self, sim: &mut StableSim<'_>, _rng_seed: u64, caster_id: usize, input: InputTargetV1) {
        crate::logger::log(&format!("[PARAGON] Basic attack fired: caster={} target={}", caster_id, input.target_id));
        let Some(atk) = sim.get_entity(caster_id).map(|e| e.stat().attack) else { return };
        sim.deal_damage_typed(caster_id, input.target_id, atk, DamageTypeV1::Ad, AttackTypeV1::BaseAttack);
    }

    fn expected_damage(&self, caster_stat: &StatV1) -> (usize, usize) {
        (caster_stat.attack, 0)
    }
}

#[derive(Debug)]
pub struct ParagonWrathgraspEffect;
impl StableEffectType for ParagonWrathgraspEffect {
    fn apply(&self, sim: &mut StableSim<'_>, _rng_seed: u64, caster_id: usize, input: InputTargetV1) {
        crate::logger::log(&format!("[PARAGON] Wrathgrasp fired: caster={} target={}", caster_id, input.target_id));
        // Yank the target toward the caster, then land the impact hit.
        sim.entity_grab(caster_id, input.target_id, 4000, 20);
        let Some(atk) = sim.get_entity(caster_id).map(|e| e.stat().attack) else { return };
        let amount = (atk as i64 * 140 / 100).max(0) as usize; // 140% AD, placeholder
        sim.deal_damage_typed(caster_id, input.target_id, amount, DamageTypeV1::Ad, AttackTypeV1::Skill);
        sim.play_view_effect("paragon_grab_impact", caster_id, &InputTargetV1::target(input.target_id), 15000, 500, 30);
    }

    fn expected_damage(&self, caster_stat: &StatV1) -> (usize, usize) {
        (caster_stat.attack * 140 / 100, 0)
    }

    fn expected_move_distance(&self) -> Option<(usize, u64)> {
        Some((20, 4000)) // ticks, speed — hints the AI this displaces the target
    }
}

#[derive(Debug)]
pub struct ParagonShatterChainEffect;
impl StableEffectType for ParagonShatterChainEffect {
    fn apply(&self, sim: &mut StableSim<'_>, _rng_seed: u64, caster_id: usize, input: InputTargetV1) {
        crate::logger::log(&format!("[PARAGON] Shatter Chain fired: caster={} target={}", caster_id, input.target_id));
        // NOTE: single-stage Bind for now, not the two-stage Fear->Bind
        // chain from the design — sequential apply_cc queuing behavior is
        // unconfirmed, shipping the reliable version first.
        let cc = CcV1 {
            kind: CcKindV1::Bind.code(),
            tick: 45, // ~0.75s placeholder
            dx: 0,
            dy: 0,
            speed: 0,
            target: 0,
            name_buf: [0u8; BUFF_NAME_CAP],
            name_len: 0,
        };
        sim.apply_cc(input.target_id, &cc);

        let Some(atk) = sim.get_entity(caster_id).map(|e| e.stat().attack) else { return };
        let amount = (atk as i64 * 90 / 100).max(0) as usize; // 90% AD, placeholder
        sim.deal_damage_typed(caster_id, input.target_id, amount, DamageTypeV1::Ad, AttackTypeV1::Skill);
        sim.play_view_effect("paragon_shatter_chain", caster_id, &InputTargetV1::target(input.target_id), 15000, 500, 30);
    }

    fn expected_damage(&self, caster_stat: &StatV1) -> (usize, usize) {
        (caster_stat.attack * 90 / 100, 0)
    }

    fn expected_cc_time(&self) -> Option<usize> {
        Some(45)
    }
}

#[derive(Debug)]
pub struct ParagonGodlessEffect;
impl StableEffectType for ParagonGodlessEffect {
    fn apply(&self, sim: &mut StableSim<'_>, _rng_seed: u64, caster_id: usize, _input: InputTargetV1) {
        crate::logger::log(&format!("[PARAGON] Godless fired: caster={}", caster_id));
        sim.play_view_effect("paragon_godless_aura", caster_id, &InputTargetV1::target(caster_id), 0, 0, 240);
        // Immortal, CC-immune for the channel duration. damage_reflect
        // deliberately omitted — field name/type not yet confirmed.
        let mut buff = BuffV1::timed("paragon_godless", 240); // ~4s placeholder
        buff.undying = true;
        buff.cc_immune = true;
        buff.attack_mult = 60; // assumed +60%, placeholder
        crate::state::add_buff(sim, caster_id, &buff, true, false, 1, true);

        // Register the DoT channel against every living enemy in range —
        // driver.rs's on_match_tick applies the actual damage each tick.
        let Some((team, (cx, cy))) = sim.get_entity(caster_id).map(|e| (e.team(), e.pos())) else { return };
        let radius_sq: u64 = 9000 * 9000; // placeholder
        let count = sim.entity_count();
        let mut targets = Vec::new();
        for i in 0..count {
            if let Some(other) = sim.entity_at(i) {
                if other.is_alive() && other.team() != team {
                    let (ex, ey) = other.pos();
                    let dx = ex as i64 - cx as i64;
                    let dy = ey as i64 - cy as i64;
                    if (dx * dx + dy * dy) as u64 <= radius_sq {
                        targets.push(other.id());
                    }
                }
            }
        }
        for target_id in targets {
            crate::state::register_periodic(sim, caster_id, target_id, 1, 240, 20, true, false, 1, true);
        }
    }

    fn expected_buff(&self, _caster_stat: &StatV1) -> Option<BuffV1> {
        let mut buff = BuffV1::timed("paragon_godless", 240);
        buff.undying = true;
        buff.cc_immune = true;
        Some(buff)
    }
}