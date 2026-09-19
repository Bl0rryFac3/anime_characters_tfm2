use mod_api_stable::*;

#[derive(Debug)]
struct ForgeSpawnAttackGate;

impl StableEffectType for ForgeSpawnAttackGate {
    fn apply(&self, sim: &mut StableSim<'_>, _rng_seed: u64, caster_id: usize, input: InputTargetV1) {
        if input.kind == InputTargetKindV1::Target.code() {
            crate::state::maintain_spawn_attack_gate(sim, caster_id, input.target_id);
        }
    }
}

// ============================================================
// NARUTO — clone-spawning effects (confirmed working, restored
// after Rust Forge's Export wiped this file back to the stub)
// ============================================================

fn clone_loadout(caster_stat: &StatV1) -> (StatV1, UnitAttackV1) {
    let clone_stat = StatV1 {
        attack: (caster_stat.attack as f32 * 0.40) as usize,
        magic_power: 0,
        hp: (caster_stat.hp as f32 * 0.25) as usize,
        defence: caster_stat.defence,
        magic_resistance: caster_stat.magic_resistance,
        move_speed: caster_stat.move_speed,
        hp_regen: caster_stat.hp_regen,
        stack: 0,
        crit_chance: caster_stat.crit_chance,
    };

    let clone_attack = UnitAttackV1 {
        attack_ratio: 100,
        attack: 0,
        range: 6000,
        cooltime: 40,
        duration: 20,
        start_timing: 10,
        cancelable: false,
        attack_type: AttackTypeV1::BaseAttack.code(),
    };

    (clone_stat, clone_attack)
}

#[derive(Debug)]
struct NarutoSpawnOneClone;

impl StableEffectType for NarutoSpawnOneClone {
    fn apply(&self, sim: &mut StableSim<'_>, _rng_seed: u64, caster_id: usize, _input: InputTargetV1) {
        let Some((team, (cx, cy), stat)) = (|| {
            let caster = sim.get_entity(caster_id)?;
            Some((caster.team(), caster.pos(), caster.stat()))
        })() else { return };

        let (clone_stat, clone_attack) = clone_loadout(&stat);
        sim.spawn_unit("naruto", caster_id, team, cx + 2000, cy, 300, &clone_stat, &clone_attack);
    }
}

#[derive(Debug)]
struct NarutoSpawnTwoClones;

impl StableEffectType for NarutoSpawnTwoClones {
    fn apply(&self, sim: &mut StableSim<'_>, _rng_seed: u64, caster_id: usize, _input: InputTargetV1) {
        let Some((team, (cx, cy), stat)) = (|| {
            let caster = sim.get_entity(caster_id)?;
            Some((caster.team(), caster.pos(), caster.stat()))
        })() else { return };

        let (clone_stat, clone_attack) = clone_loadout(&stat);
        sim.spawn_unit("naruto", caster_id, team, cx + 2000, cy + 1500, 300, &clone_stat, &clone_attack);
        sim.spawn_unit("naruto", caster_id, team, cx + 2000, cy.saturating_sub(1500), 300, &clone_stat, &clone_attack);
    }
}

#[derive(Debug)]
struct NarutoSpawnCloneSwarm;

impl StableEffectType for NarutoSpawnCloneSwarm {
    fn apply(&self, sim: &mut StableSim<'_>, _rng_seed: u64, caster_id: usize, _input: InputTargetV1) {
        let Some((team, (cx, cy), stat)) = (|| {
            let caster = sim.get_entity(caster_id)?;
            Some((caster.team(), caster.pos(), caster.stat()))
        })() else { return };

        let (clone_stat, clone_attack) = clone_loadout(&stat);
        sim.spawn_unit("naruto", caster_id, team, cx + 2500, cy + 1500, 300, &clone_stat, &clone_attack);
        sim.spawn_unit("naruto", caster_id, team, cx + 2500, cy.saturating_sub(1500), 300, &clone_stat, &clone_attack);
        sim.spawn_unit("naruto", caster_id, team, cx.saturating_sub(2000), cy + 2000, 300, &clone_stat, &clone_attack);
        sim.spawn_unit("naruto", caster_id, team, cx.saturating_sub(2000), cy.saturating_sub(2000), 300, &clone_stat, &clone_attack);
    }
}

// ============================================================
// SAITAMA — combo/serious-mode effects
// ============================================================

const SERIOUS_BUFF: &str = "saitama_serious_mode";
const COMBO_COUNTER: &str = "saitama_combo";

#[derive(Debug)]
struct SaitamaComboTick;
impl StableEffectType for SaitamaComboTick {
    fn apply(&self, sim: &mut StableSim<'_>, _rng_seed: u64, caster_id: usize, _input: InputTargetV1) {
        crate::state::add_counter(sim, caster_id, COMBO_COUNTER, 1);
    }
}

/// Shared: normal-mode consecutive punches, scaled by combo tier.
fn saitama_flurry(sim: &mut StableSim<'_>, caster_id: usize, target_id: usize, hits: u32, base_ratio: i64) {
    let tier = crate::state::counter_value(sim, caster_id, COMBO_COUNTER).min(20);
    let Some(atk) = sim.get_entity(caster_id).map(|e| e.stat().attack) else { return };
    let ratio = base_ratio + tier * 15;
    let amount = (atk as i64 * ratio / 100 / hits as i64).max(1) as usize;
    for _ in 0..hits {
        sim.deal_damage_typed(caster_id, target_id, amount, DamageTypeV1::Ad, AttackTypeV1::Skill);
    }
}

/// Shared: single-target serious-mode punch, scaled by combo tier + serious bonus.
fn saitama_serious_punch(sim: &mut StableSim<'_>, caster_id: usize, target_id: usize, base_ratio: i64) {
    let tier = crate::state::counter_value(sim, caster_id, COMBO_COUNTER).min(20);
    let Some(atk) = sim.get_entity(caster_id).map(|e| e.stat().attack) else { return };
    let ratio = base_ratio + tier * 100 + 800; // one-punch tier: lethal to most champions even at 0 stacks
    let amount = (atk as i64 * ratio / 100).max(0) as usize;
    sim.deal_damage_typed(caster_id, target_id, amount, DamageTypeV1::Ad, AttackTypeV1::Skill);
    sim.play_view_effect("saitama_serious_punch_impact", caster_id, &InputTargetV1::target(target_id), 0, 0, 30);
}

#[derive(Debug)]
struct SaitamaSkillPunch;
impl StableEffectType for SaitamaSkillPunch {
    fn apply(&self, sim: &mut StableSim<'_>, _rng_seed: u64, caster_id: usize, input: InputTargetV1) {
        if crate::helpers::has_buff(sim, caster_id, SERIOUS_BUFF) {
            saitama_serious_punch(sim, caster_id, input.target_id, 80);
        } else {
            saitama_flurry(sim, caster_id, input.target_id, 3, 90); // 3 one-hand hits
        }
    }
}

#[derive(Debug)]
struct SaitamaSkillPunch2;
impl StableEffectType for SaitamaSkillPunch2 {
    fn apply(&self, sim: &mut StableSim<'_>, _rng_seed: u64, caster_id: usize, input: InputTargetV1) {
        if crate::helpers::has_buff(sim, caster_id, SERIOUS_BUFF) {
            // Serious mode: table-flip projectile that stuns on hit.
            let Some((cx, cy, team)) = sim.get_entity(caster_id).map(|e| (e.pos().0, e.pos().1, e.team())) else { return };
            let spec = ProjectileSpawnV1 {
                caster_id,
                team,
                x: cx,
                y: cy,
                radius: 500,
                speed: 3000,
                move_kind: ProjectileMoveKindV1::Target.code(),
                target_id: input.target_id,
                target_x: 0,
                target_y: 0,
                penetrate: false,
                attack_type: AttackTypeV1::Skill.code(),
                casting_type: CastingTypeV1::Targeting.code(),
                casting_target: CastingTargetV1::Enemy.code(),
            };
            sim.spawn_projectile("saitama_table_flip", "saitama_table_flip_hit", &spec);
        } else {
            saitama_flurry(sim, caster_id, input.target_id, 2, 100); // 2 two-hand hits
        }
    }
}

/// Fires when the table-flip projectile connects — apply the stun here.
/// Referenced as the projectile's `effect_name` (2nd spawn_projectile arg).
#[derive(Debug)]
struct SaitamaTableFlipHit;
impl StableEffectType for SaitamaTableFlipHit {
    fn apply(&self, sim: &mut StableSim<'_>, _rng_seed: u64, caster_id: usize, input: InputTargetV1) {
        let tier = crate::state::counter_value(sim, caster_id, COMBO_COUNTER).min(20);
        let Some(atk) = sim.get_entity(caster_id).map(|e| e.stat().attack) else { return };
        let amount = (atk as i64 * (150 + tier * 20) / 100).max(0) as usize;
        sim.deal_damage_typed(caster_id, input.target_id, amount, DamageTypeV1::Ad, AttackTypeV1::Skill);

        let cc = CcV1 {
            kind: CcKindV1::Stun.code(),
            tick: 60, // ~1s, tune once confirmed against real tick rate
            dx: 0, dy: 0, speed: 0,
            target: 0,
            name_buf: [0u8; BUFF_NAME_CAP],
            name_len: 0,
        };
        sim.apply_cc(input.target_id, &cc);
    }
}

/// Ult: first cast activates Serious Mode (no refresh if recast while active).
/// Second cast while already serious = Death Punch, and ends Serious Mode early.
#[derive(Debug)]
struct SaitamaSeriousMode;
impl StableEffectType for SaitamaSeriousMode {
    fn apply(&self, sim: &mut StableSim<'_>, _rng_seed: u64, caster_id: usize, input: InputTargetV1) {
        if crate::helpers::has_buff(sim, caster_id, SERIOUS_BUFF) {
            // Death Punch: capture the target's position BEFORE dealing
            // damage — a hit this large can be lethal, and if the target
            // entity is removed same-tick, an entity-targeted view effect
            // has nothing left to resolve against and silently fails to
            // render. Position-based targeting avoids that entirely.
            let target_pos = sim.get_entity(input.target_id).map(|e| e.pos());
            let tier = crate::state::counter_value(sim, caster_id, COMBO_COUNTER).min(20);
            let Some(atk) = sim.get_entity(caster_id).map(|e| e.stat().attack) else { return };
            let amount = (atk as i64 * (600 + tier * 60) / 100).max(0) as usize;
            sim.deal_damage_typed(caster_id, input.target_id, amount, DamageTypeV1::Ad, AttackTypeV1::Skill);
            if let Some((tx, ty)) = target_pos {
                sim.play_view_effect("saitama_death_punch_impact", caster_id, &InputTargetV1::pos(tx, ty), 0, 0, 45);
            }
            crate::state::remove_buff(sim, caster_id, SERIOUS_BUFF);
        } else {
            // Activate: timed buff, no-refresh (this branch only runs while
            // the buff is absent — recasting mid-duration always hits the
            // Death Punch branch above instead).
            //
            // Without a cooldown cut here, Death Punch is structurally
            // unreachable: Serious Mode lasts 300 ticks but Ult's own
            // cooldown is 900 — the buff always expires 600 ticks before
            // Ult could ever be recast. Slashing the cooldown while this
            // buff is up is what actually makes a same-window recast
            // possible at all.
            let mut buff = BuffV1::timed(SERIOUS_BUFF, 450); // real balance value — widened from original 300, well short of the 1800 testing value
            buff.attack_mult = 150; // assumed +150%, verify in-game
            // NOTE: -80 was tried first and appears to have over-corrected —
            // Ult's cooldown collapsed low enough that the AI chain-cast it
            // repeatedly (activate -> Death Punch -> activate -> ...) instead
            // of ever landing a punch. -35 is a much more conservative retry;
            // still unconfirmed exact units, tune further from here.
            buff.ult_cooldown_mult = -35;
            crate::state::add_buff(sim, caster_id, &buff, true, false, 1, true);
        }
    }
}

pub fn register(reg: &mut StableMod) {
    reg.add_native_effect("__forge_spawn_attack_gate", ForgeSpawnAttackGate);

    // Naruto
    reg.add_native_effect("naruto_spawn_one_clone", NarutoSpawnOneClone);
    reg.add_native_effect("naruto_spawn_two_clones", NarutoSpawnTwoClones);
    reg.add_native_effect("naruto_spawn_clone_swarm", NarutoSpawnCloneSwarm);

    // Saitama
    reg.add_native_effect("saitama_combo_tick", SaitamaComboTick);
    reg.add_native_effect("saitama_skill_punch", SaitamaSkillPunch);
    reg.add_native_effect("saitama_skill_punch2", SaitamaSkillPunch2);
    reg.add_native_effect("saitama_table_flip_hit", SaitamaTableFlipHit);
    reg.add_native_effect("saitama_serious_mode", SaitamaSeriousMode);

    // Paragon
    reg.add_native_effect("paragon_wrathgrasp", crate::paragon::ParagonWrathgraspEffect);
    reg.add_native_effect("paragon_shatter_chain", crate::paragon::ParagonShatterChainEffect);
    reg.add_native_effect("paragon_godless", crate::paragon::ParagonGodlessEffect);

    
}