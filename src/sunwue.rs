// src/sunwue.rs - Sun Wue sword cultivator
// Realm visuals: one aura at a time, restored on the frame he is alive.
// Skill 1 visuals: crescent / line / circle are three different effect names.
use mod_api_stable::*;

pub struct SunWue;

impl StableChampion for SunWue {
    fn id(&self) -> String { "sunwue".to_string() }
    fn name(&self) -> String { "Sun Wue".to_string() }
    fn category(&self) -> ChampionCategoryV1 { ChampionCategoryV1::Range }
    fn tags(&self) -> Vec<ChampionTagV1> { vec![ChampionTagV1::Ad, ChampionTagV1::Melee, ChampionTagV1::Cc] }
    fn stat(&self) -> StatV1 {
        StatV1 { attack: 105, magic_power: 0, hp: 1500, defence: 42, magic_resistance: 34, move_speed: 1100, hp_regen: 4, stack: 0, crit_chance: 10 }
    }
    fn growth(&self) -> StatV1 {
        StatV1 { attack: 18, magic_power: 0, hp: 120, defence: 5, magic_resistance: 4, move_speed: 4, hp_regen: 1, stack: 0, crit_chance: 0 }
    }
    fn attack(&self) -> Box<dyn StableAction> { Box::new(SunWueAttack) }
    fn skill(&self) -> Box<dyn StableAction> { Box::new(SunWueSkill) }
    fn skill2(&self) -> Box<dyn StableAction> { Box::new(SunWueSkill2) }
    fn ult(&self) -> Option<Box<dyn StableAction>> { Some(Box::new(SunWueUlt)) }
    fn passive(&self) -> Option<Box<dyn StablePassive>> { Some(Box::new(SunWuePassive::default())) }
}

pub const TIER_FOUNDATION: &str = "sunwue_foundation";
pub const TIER_GOLDEN_CORE: &str = "sunwue_golden_core";
pub const TIER_NASCENT_SOUL: &str = "sunwue_nascent_soul";

// Realms are stack thresholds, not a match clock. One qi per second only while
// an enemy is within 40 tiles. A kill adds 10. Fountain time adds nothing.
const QI_COUNTER: &str = "sunwue_qi_stacks";
const UNDYING_FLAG: &str = "sunwue_undying_given";
const STACKS_FOUNDATION: usize = 12;
const STACKS_GOLDEN: usize = 80;
const STACKS_NASCENT: usize = 250;
const QI_TICKS: u32 = 60;
const CULTIVATION_RANGE_SQ: u64 = 40_000 * 40_000;

#[derive(Clone)] struct SunWueAttack;
impl StableAction for SunWueAttack {
    fn clone_box(&self) -> Box<dyn StableAction> { Box::new(self.clone()) }
    fn action_name(&self) -> String { "attack".to_string() }
    fn duration(&self) -> usize { 20 }
    fn cooltime(&self, _s: &StatV1, _l: usize) -> usize { 50 }
    fn casting_target(&self) -> CastingTargetV1 { CastingTargetV1::Enemy }
    fn effect(&self) -> Option<StableEffectSpec> {
        Some(StableEffectSpec { range: 15000, growth_range: 0, start_timing: 6, casting: CastingTypeV1::Targeting, target: CastingTargetV1::Enemy, attack_type: AttackTypeV1::BaseAttack, effect: Box::new(SunWueAttackEffect) })
    }
}

#[derive(Clone)] struct SunWueSkill;
impl StableAction for SunWueSkill {
    fn clone_box(&self) -> Box<dyn StableAction> { Box::new(self.clone()) }
    fn action_name(&self) -> String { "skill".to_string() }
    fn duration(&self) -> usize { 24 }
    fn cooltime(&self, _s: &StatV1, _l: usize) -> usize { 80 }
    fn casting_target(&self) -> CastingTargetV1 { CastingTargetV1::Enemy }
    fn effect(&self) -> Option<StableEffectSpec> {
        Some(StableEffectSpec { range: 65000, growth_range: 0, start_timing: 5, casting: CastingTypeV1::Targeting, target: CastingTargetV1::Enemy, attack_type: AttackTypeV1::Skill, effect: Box::new(SunWueSwordQiEffect) })
    }
}

#[derive(Clone)] struct SunWueSkill2;
impl StableAction for SunWueSkill2 {
    fn clone_box(&self) -> Box<dyn StableAction> { Box::new(self.clone()) }
    fn action_name(&self) -> String { "skill2".to_string() }
    fn duration(&self) -> usize { 30 }
    fn cooltime(&self, _s: &StatV1, _l: usize) -> usize { 200 }
    fn casting_target(&self) -> CastingTargetV1 { CastingTargetV1::Enemy }
    fn effect(&self) -> Option<StableEffectSpec> {
        Some(StableEffectSpec { range: 9000, growth_range: 0, start_timing: 6, casting: CastingTypeV1::None, target: CastingTargetV1::Enemy, attack_type: AttackTypeV1::Skill, effect: Box::new(SunWueBarrierEffect) })
    }
}

#[derive(Clone)] struct SunWueUlt;
impl StableAction for SunWueUlt {
    fn clone_box(&self) -> Box<dyn StableAction> { Box::new(self.clone()) }
    fn action_name(&self) -> String { "ult".to_string() }
    fn duration(&self) -> usize { 40 }
    fn cooltime(&self, _s: &StatV1, _l: usize) -> usize { 1200 }
    fn casting_target(&self) -> CastingTargetV1 { CastingTargetV1::Enemy }
    fn effect(&self) -> Option<StableEffectSpec> {
        Some(StableEffectSpec { range: 45000, growth_range: 0, start_timing: 8, casting: CastingTypeV1::None, target: CastingTargetV1::Enemy, attack_type: AttackTypeV1::Skill, effect: Box::new(SunWueMyriadSlamSelf) })
    }
}

#[derive(Debug)] struct SunWueAttackEffect;
impl StableEffectType for SunWueAttackEffect {
    fn apply(&self, sim: &mut StableSim<'_>, _rng: u64, caster: usize, input: InputTargetV1) {
        let atk = sim.get_entity(caster).map(|e| e.stat().attack).unwrap_or(85);
        sim.deal_damage_typed(caster, input.target_id, atk, DamageTypeV1::Ad, AttackTypeV1::BaseAttack);
        if !cue_realm_sfx(sim, caster) {
            play_sfx_on(sim, caster, "sunwue_attack");
        }
    }
    fn expected_damage(&self, s: &StatV1) -> (usize, usize) { (s.attack, 0) }
}

#[derive(Debug)] pub struct SunWueSwordQiEffect;
impl StableEffectType for SunWueSwordQiEffect {
    fn apply(&self, sim: &mut StableSim<'_>, _rng: u64, caster: usize, input: InputTargetV1) {
        let atk = sim.get_entity(caster).map(|e| e.stat().attack).unwrap_or(85);
        if crate::helpers::has_buff(sim, caster, TIER_NASCENT_SOUL) {
            circular_slash(sim, caster, input.target_id, atk);
            play_skill1_sfx(sim, caster, "sunwue_skill_circle");
        } else if crate::helpers::has_buff(sim, caster, TIER_GOLDEN_CORE) {
            launch_slash(sim, caster, input.target_id, "sunwue_sword_qi_line_hit", true);
            play_skill1_sfx(sim, caster, "sunwue_skill_line");
        } else {
            launch_slash(sim, caster, input.target_id, "sunwue_sword_qi_hit", false);
            play_skill1_sfx(sim, caster, "sunwue_skill");
        }
    }
    fn expected_damage(&self, s: &StatV1) -> (usize, usize) { (s.attack * 160 / 100, 0) }
}

/// Foundation and Golden Core Skill 1. This is a real projectile: the engine
/// moves it, and the registered hit effect damages whoever it touches.
/// `penetrate` is false for the crescent (first enemy) and true for the line.
fn launch_slash(sim: &mut StableSim<'_>, caster: usize, target_id: usize, hit_effect: &str, penetrate: bool) {
    let Some((team, (cx, cy))) = sim.get_entity(caster).map(|unit| (unit.team(), unit.pos())) else { return };
    let Some((tx, ty)) = sim.get_entity(target_id).map(|unit| unit.pos()) else { return };
    let (ex, ey) = flight_point(cx, cy, tx, ty);
    let spec = ProjectileSpawnV1 {
        caster_id: caster,
        team,
        x: cx,
        y: cy,
        // SDK calibration: radius 10000 is about one champion. 12000 lets the
        // slash connect instead of flying through a gap beside the target.
        // 52000 units over 36 ticks matches the picture below.
        radius: 12_000,
        speed: 1_805,
        move_kind: ProjectileMoveKindV1::Linear.code(),
        target_id,
        target_x: ex,
        target_y: ey,
        penetrate,
        attack_type: AttackTypeV1::Skill.code(),
        casting_type: CastingTypeV1::Targeting.code(),
        casting_target: CastingTargetV1::Enemy.code(),
    };
    // The right-facing sheet points along +X. The projectile renderer mirrors
    // that to the flight direction. A pre-flipped left sheet gets mirrored
    // again, so the point turns back toward him.
    let view_name = if penetrate { "sunwue_sword_qi_line" } else { "sunwue_sword_qi" };
    sim.spawn_projectile(view_name, hit_effect, &spec);
}

/// 52 tiles past the caster, through the target. Linear projectiles fly to this
/// point instead of stopping on the enemy.
fn flight_point(cx: u64, cy: u64, tx: u64, ty: u64) -> (u64, u64) {
    const FLIGHT: f64 = 65_000.0;
    let dx = tx as i64 - cx as i64;
    let dy = ty as i64 - cy as i64;
    let len = ((dx * dx + dy * dy) as f64).sqrt().max(1.0);
    let ex = cx as i64 + (dx as f64 / len * FLIGHT) as i64;
    let ey = cy as i64 + (dy as f64 / len * FLIGHT) as i64;
    (ex.max(0) as u64, ey.max(0) as u64)
}

/// The accepted ring is about seven bodies across. 10000 is one champion, so
/// the hit circle has to be 70000 or a minion standing in the sprite is missed.
const AROUND_SQ: u64 = 70_000 * 70_000;

fn consider_around(targets: &mut Vec<usize>, caster: usize, team: usize, id: usize, alive: bool, other_team: usize, tower: bool) {
    if id == caster || id == usize::MAX || !alive || tower || other_team == team || targets.contains(&id) {
        return;
    }
    targets.push(id);
}

fn enemies_near(sim: &StableSim<'_>, caster: usize, origin: (u64, u64), reach_sq: u64) -> Vec<usize> {
    let Some(origin_entity) = sim.get_entity(caster) else { return Vec::new(); };
    let team = origin_entity.team();
    let mut targets = Vec::new();
    for i in 0..sim.entity_count() {
        let Some(other) = sim.entity_at(i) else { continue };
        if crate::helpers::distance_sq(origin, other.pos()) > reach_sq {
            continue;
        }
        consider_around(&mut targets, caster, team, other.id(), other.is_alive(), other.team(), other.is_tower());
    }
    for i in 0..sim.player_count() {
        let Some(player) = sim.player_at(i) else { continue };
        let Some(champ) = player.champion() else { continue };
        if crate::helpers::distance_sq(origin, champ.pos()) > reach_sq {
            continue;
        }
        consider_around(&mut targets, caster, team, champ.id(), champ.is_alive(), champ.team(), false);
    }
    targets
}

fn enemies_around(sim: &StableSim<'_>, caster: usize) -> Vec<usize> {
    let Some(origin) = sim.get_entity(caster).map(|unit| unit.pos()) else { return Vec::new(); };
    enemies_near(sim, caster, origin, AROUND_SQ)
}

fn circular_slash(sim: &mut StableSim<'_>, caster: usize, target_id: usize, atk: usize) {
    let dmg = atk * 180 / 100;
    let Some(origin) = sim.get_entity(target_id).map(|unit| unit.pos()) else { return };
    // Close to the aimed enemy: 10000 is one champion, melee spacing is 15000.
    let mut targets = enemies_near(sim, caster, origin, 35_000 * 35_000);
    let caster_team = sim.get_entity(caster).map(|unit| unit.team());
    let locked_enemy = sim.get_entity(target_id).map(|unit| unit.is_alive() && Some(unit.team()) != caster_team).unwrap_or(false);
    if locked_enemy && !targets.contains(&target_id) {
        targets.push(target_id);
    }
    for tid in targets {
        sim.deal_damage(caster, tid, dmg, 0, AttackTypeV1::Skill);
    }
    sim.play_view_effect("sunwue_sword_qi_circle", caster, &InputTargetV1::target(target_id), 0, 500, 48);
}

/// Hit effects for the traveling slashes. Both names must be registered in
/// native_effects/mod.rs or the projectile flies and deals no damage.
#[derive(Debug)] pub struct SunWueSwordQiHit;
impl StableEffectType for SunWueSwordQiHit {
    fn apply(&self, sim: &mut StableSim<'_>, _rng: u64, caster: usize, input: InputTargetV1) {
        let atk = sim.get_entity(caster).map(|unit| unit.stat().attack).unwrap_or(85);
        sim.deal_damage_typed(caster, input.target_id, atk * 140 / 100, DamageTypeV1::Ad, AttackTypeV1::Skill);
    }
    fn expected_damage(&self, s: &StatV1) -> (usize, usize) { (s.attack * 140 / 100, 0) }
}

#[derive(Debug)] pub struct SunWueSwordQiLineHit;
impl StableEffectType for SunWueSwordQiLineHit {
    fn apply(&self, sim: &mut StableSim<'_>, _rng: u64, caster: usize, input: InputTargetV1) {
        let atk = sim.get_entity(caster).map(|unit| unit.stat().attack).unwrap_or(85);
        sim.deal_damage_typed(caster, input.target_id, atk * 160 / 100, DamageTypeV1::Ad, AttackTypeV1::Skill);
    }
    fn expected_damage(&self, s: &StatV1) -> (usize, usize) { (s.attack * 160 / 100, 0) }
}

#[derive(Debug)] pub struct SunWueBarrierEffect;
impl StableEffectType for SunWueBarrierEffect {
    fn apply(&self, sim: &mut StableSim<'_>, _rng: u64, caster: usize, _input: InputTargetV1) {
        let mut buff = BuffV1::timed("sunwue_barrier", 180);
        buff.defence = 30;
        buff.magic_resistance = 30;
        buff.cc_immune = true;
        crate::state::add_buff(sim, caster, &buff, false, false, 1, true);
        sim.play_view_effect("sunwue_barrier", caster, &InputTargetV1::target(caster), 0, 0, 180);
        play_sfx_on(sim, caster, "sunwue_skill2");
    }
    fn expected_buff(&self, _s: &StatV1) -> Option<BuffV1> {
        let mut b = BuffV1::timed("sunwue_barrier", 180);
        b.defence = 30;
        Some(b)
    }
}

#[derive(Debug)] pub struct SunWueMyriadSlamSelf;
impl StableEffectType for SunWueMyriadSlamSelf {
    fn apply(&self, sim: &mut StableSim<'_>, _rng: u64, caster: usize, _input: InputTargetV1) {
        let atk = sim.get_entity(caster).map(|e| e.stat().attack).unwrap_or(85);
        let targets = enemies_around(sim, caster);
        let has_nascent = crate::helpers::has_buff(sim, caster, TIER_NASCENT_SOUL);
        let ratio = if has_nascent { 600 } else { 300 };
        let dmg = atk * ratio / 100;
        for tid in targets {
            sim.deal_damage(caster, tid, dmg, 0, AttackTypeV1::Skill);
        }
        sim.play_view_effect("sunwue_myriad_slam", caster, &InputTargetV1::target(caster), 0, 800, 30);
        play_sfx_on(sim, caster, "sunwue_ult");
        let mut buff = BuffV1::timed("sunwue_slam_buff", 180);
        buff.attack_mult = 30;
        crate::state::add_buff(sim, caster, &buff, false, false, 1, true);
    }
    fn expected_damage(&self, s: &StatV1) -> (usize, usize) { (s.attack * 300 / 100, 0) }
}

#[derive(Debug, Clone, Default)]
pub struct SunWuePassive { stacks: usize, tick_accumulator: u32 }

impl StablePassive for SunWuePassive {
    fn clone_box(&self) -> Box<dyn StablePassive> { Box::new(self.clone()) }

    fn on_spawn(&mut self, sim: &mut StableSim<'_>, _player: usize, entity: usize) {
        self.stacks = remembered_stacks(sim, entity, self.stacks);
        sync_realm(sim, entity, self.stacks);
    }

    fn on_update(&mut self, sim: &mut StableSim<'_>, _rng: u64, _player: usize, entity: usize) {
        self.stacks = remembered_stacks(sim, entity, self.stacks);
        sync_realm(sim, entity, self.stacks);
        let alive = sim.get_entity(entity).map(|unit| unit.is_alive()).unwrap_or(false);
        if !alive || !enemy_in_cultivation_range(sim, entity) {
            return;
        }
        self.tick_accumulator = self.tick_accumulator.saturating_add(1);
        if self.tick_accumulator < QI_TICKS {
            return;
        }
        self.tick_accumulator = 0;
        gain_qi(self, sim, entity, 1);
    }

    fn on_kill(&mut self, sim: &mut StableSim<'_>, _p: usize, entity: usize, _victim: usize) {
        gain_qi(self, sim, entity, 10);
    }
}


fn enemy_in_cultivation_range(sim: &StableSim<'_>, entity: usize) -> bool {
    let Some((team, (cx, cy))) = sim.get_entity(entity).map(|unit| (unit.team(), unit.pos())) else {
        return false;
    };
    for index in 0..sim.entity_count() {
        let Some(other) = sim.entity_at(index) else { continue; };
        if !other.is_alive() || other.team() == team {
            continue;
        }
        let (ex, ey) = other.pos();
        let dx = ex as i64 - cx as i64;
        let dy = ey as i64 - cy as i64;
        if (dx * dx + dy * dy) as u64 <= CULTIVATION_RANGE_SQ {
            return true;
        }
    }
    false
}

fn remembered_stacks(sim: &StableSim<'_>, entity: usize, local: usize) -> usize {
    local.max(crate::state::counter_value(sim, entity, QI_COUNTER).max(0) as usize)
}

fn play_sfx_on(sim: &mut StableSim<'_>, caster: usize, name: &str) {
    sim.play_sfx(name, caster, &InputTargetV1::target(caster));
}

/// Skill 1 can be entered twice in one cast if the data file and the Rust
/// action both fire. The second entry must not play another slash sound.
fn play_skill1_sfx(sim: &mut StableSim<'_>, caster: usize, name: &str) {
    let tick = sim.tick() as i64;
    let last = crate::state::counter_value(sim, caster, "sunwue_skill1_sfx_tick");
    if last == tick {
        return;
    }
    crate::state::add_counter(sim, caster, "sunwue_skill1_sfx_tick", tick - last);
    play_sfx_on(sim, caster, name);
}

/// One sound, on a basic attack, after the aura is up. Skill 1 does not call
/// this, so the slash and the chime never play on the same cast. Nascent Soul
/// plays the chime first, then the undying tone on a later attack.
fn cue_realm_sfx(sim: &mut StableSim<'_>, entity: usize) -> bool {
    if crate::helpers::has_buff(sim, entity, TIER_NASCENT_SOUL) {
        if !crate::state::flag_is_set(sim, entity, "sunwue_heard_nascent") {
            crate::state::set_flag(sim, entity, "sunwue_heard_nascent", true);
            play_sfx_on(sim, entity, "sunwue_realm");
            return true;
        }
        if !crate::state::flag_is_set(sim, entity, "sunwue_heard_undying") {
            crate::state::set_flag(sim, entity, "sunwue_heard_undying", true);
            play_sfx_on(sim, entity, "sunwue_undying");
            return true;
        }
        return false;
    }
    if crate::helpers::has_buff(sim, entity, TIER_GOLDEN_CORE) {
        if !crate::state::flag_is_set(sim, entity, "sunwue_heard_golden") {
            crate::state::set_flag(sim, entity, "sunwue_heard_golden", true);
            play_sfx_on(sim, entity, "sunwue_realm");
            return true;
        }
        return false;
    }
    if crate::helpers::has_buff(sim, entity, TIER_FOUNDATION)
        && !crate::state::flag_is_set(sim, entity, "sunwue_heard_foundation")
    {
        crate::state::set_flag(sim, entity, "sunwue_heard_foundation", true);
        play_sfx_on(sim, entity, "sunwue_realm");
        return true;
    }
    false
}

fn gain_qi(passive: &mut SunWuePassive, sim: &mut StableSim<'_>, entity: usize, amount: usize) {
    passive.stacks = passive.stacks.saturating_add(amount);
    crate::state::add_counter(sim, entity, QI_COUNTER, amount as i64);
    passive.stacks = remembered_stacks(sim, entity, passive.stacks);
    grant_qi(sim, entity, amount);
    if passive.stacks >= STACKS_NASCENT && !crate::state::flag_is_set(sim, entity, UNDYING_FLAG) {
        crate::state::set_flag(sim, entity, UNDYING_FLAG, true);
        let mut undying = BuffV1::timed("sunwue_undying", 180);
        undying.undying = true;
        crate::state::add_buff(sim, entity, &undying, true, false, 1, true);
    }
    sync_realm(sim, entity, passive.stacks);
}

fn grant_qi(sim: &mut StableSim<'_>, entity: usize, amount: usize) {
    for _ in 0..amount {
        let mut b = BuffV1::named("sunwue_qi");
        b.attack_mult = 1;
        sim.entity_stack_buff(entity, &b, 100, false);
    }
}

fn add_realm(sim: &mut StableSim<'_>, entity: usize, name: &str, attack_mult: i32, defence: i32, move_speed_mult: i32, cc_immune: bool) {
    if crate::helpers::has_buff(sim, entity, name) {
        return;
    }
    let mut b = BuffV1::timed(name, 999_999);
    b.attack_mult = attack_mult;
    b.defence = defence;
    b.move_speed_mult = move_speed_mult;
    b.cc_immune = cc_immune;
    crate::state::add_buff(sim, entity, &b, true, false, 1, true);
}

/// Drops every realm except the one `stacks` qualifies for, including the
/// copy saved through death. `entity_remove_buff` does not do that.
fn sync_realm(sim: &mut StableSim<'_>, entity: usize, stacks: usize) {
    let tier = if stacks >= STACKS_NASCENT { 2 } else if stacks >= STACKS_GOLDEN { 1 } else if stacks >= STACKS_FOUNDATION { 0 } else { -1 };
    if tier != 0 {
        crate::state::remove_buff(sim, entity, TIER_FOUNDATION);
    }
    if tier != 1 {
        crate::state::remove_buff(sim, entity, TIER_GOLDEN_CORE);
    }
    if tier != 2 {
        crate::state::remove_buff(sim, entity, TIER_NASCENT_SOUL);
    }
    if tier == 0 {
        add_realm(sim, entity, TIER_FOUNDATION, 20, 0, 10, false);
    } else if tier == 1 {
        add_realm(sim, entity, TIER_GOLDEN_CORE, 40, 0, 0, false);
    } else if tier == 2 {
        add_realm(sim, entity, TIER_NASCENT_SOUL, 80, 30, 0, true);
    }
}
