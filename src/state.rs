use mod_api_stable::{AttackTypeV1, BuffDurationV1, BuffV1, CcKindV1, CcV1, InputTargetV1, ProjectileInfoV1, SimCtxV1, SimOriginKindV1, StableSim, StatV1};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Clone)]
struct PersistentBuff {
    buff: BuffV1,
    expires_at: Option<usize>,
}

#[derive(Default)]
struct PlayerState {
    persistent_counters: HashMap<String, i64>,
    flags: HashMap<String, bool>,
    persistent_buffs: Vec<PersistentBuff>,
    persistent_base_stat: Option<StatV1>,
    persistent_modify_stats_stack: Option<usize>,
    champion_key: Option<String>,
    last_entity_id: Option<usize>,
    was_alive: bool,
    entity_missing: bool,
    persistent_capture_count: usize,
    restore_count: usize,
    restored_buff_count: usize,
    identity_change_count: usize,
}

pub type SimId = usize;
type PlayerKey = (SimId, usize);
type EntityKey = (SimId, usize);

#[derive(Clone, Debug)]
pub struct SpawnedUnit {
    pub serial: u64,
    pub owner_id: usize,
    pub entity_id: usize,
    pub name: String,
    pub created_tick: u64,
    pub attack_enabled: bool,
    pub attack_gate_queued: bool,
}

#[derive(Default)]
struct SpawnAggregationScope {
    allow_duplicates: bool,
    additional_multiplier: f32,
    hit_counts: HashMap<usize, usize>,
    current_multipliers: HashMap<usize, f32>,
}

static STATES: OnceLock<Mutex<HashMap<PlayerKey, PlayerState>>> = OnceLock::new();
static ENTITY_OWNERS: OnceLock<Mutex<HashMap<EntityKey, usize>>> = OnceLock::new();
static SPAWNED_UNITS: OnceLock<Mutex<Vec<(SimId, SpawnedUnit)>>> = OnceLock::new();
static CANCELLED_SPAWNED_UNIT_EFFECTS: OnceLock<Mutex<Vec<(SimId, usize, usize)>>> = OnceLock::new();
static SPAWN_AGGREGATION_SCOPES: OnceLock<Mutex<HashMap<SimId, Vec<SpawnAggregationScope>>>> = OnceLock::new();
static NEXT_SPAWNED_UNIT_SERIAL: AtomicU64 = AtomicU64::new(1);
static MISSED_OWNER_LOOKUPS: AtomicUsize = AtomicUsize::new(0);

fn states() -> &'static Mutex<HashMap<PlayerKey, PlayerState>> {
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn entity_owners() -> &'static Mutex<HashMap<EntityKey, usize>> {
    ENTITY_OWNERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn spawned_units_store() -> &'static Mutex<Vec<(SimId, SpawnedUnit)>> {
    SPAWNED_UNITS.get_or_init(|| Mutex::new(Vec::new()))
}

fn cancelled_spawned_unit_effects() -> &'static Mutex<Vec<(SimId, usize, usize)>> {
    CANCELLED_SPAWNED_UNIT_EFFECTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn spawn_aggregation_scopes() -> &'static Mutex<HashMap<SimId, Vec<SpawnAggregationScope>>> {
    SPAWN_AGGREGATION_SCOPES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Returns the opaque host-state pointer backing this StableSim as a numeric
/// simulation-instance id. `SimCtxV1.state` is the host's actual per-sim
/// context, so deterministic copies with identical seeds/ticks/player ids are
/// still isolated from one another.
///
/// SAFETY / SDK COUPLING: StableSim does not publicly expose its raw SimCtxV1.
/// In the current stable SDK its first field is `raw: *mut SimCtxV1`; Forge is
/// compiled against that exact SDK source. If StableSim's private layout ever
/// changes, this is the one helper that must be updated.
#[inline]
pub fn sim_instance_id(sim: &StableSim<'_>) -> SimId {
    // StableSim currently contains one pointer-sized field plus PhantomData.
    // Refuse to dereference a guessed layout if that ever stops being true.
    if std::mem::size_of::<StableSim<'_>>() != std::mem::size_of::<*mut SimCtxV1>() {
        return 0;
    }
    unsafe {
        let raw_slot = sim as *const StableSim<'_> as *const *mut SimCtxV1;
        let raw = *raw_slot;
        if raw.is_null() { 0 } else { (*raw).state as SimId }
    }
}

#[inline]
fn player_key(sim: &StableSim<'_>, player_id: usize) -> PlayerKey {
    (sim_instance_id(sim), player_id)
}

/// Resolves the entity's owning player at apply time and binds the champion
/// identity. Used by Data champions whose native effects never go through a
/// generated on_spawn, so their persistent state is registered lazily.
pub fn register_entity(sim: &StableSim<'_>, entity_id: usize, champion_key: &str) {
    let Some(player_id) = player_for_entity(sim, entity_id) else { return; };
    init(sim, player_id, entity_id, champion_key);
}

/// Ensure match-persistent state exists for this player and bind the current
/// generated champion identity to its runtime entity.
pub fn init(sim: &StableSim<'_>, player_id: usize, entity_id: usize, champion_key: &str) {
    let sim_id = sim_instance_id(sim);
    {
        let mut guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let state = guard.entry((sim_id, player_id)).or_default();
        if matches!(state.champion_key.as_deref(), Some(key) if key != champion_key) {
            let identity_changes = state.identity_change_count.saturating_add(1);
            *state = PlayerState::default();
            state.identity_change_count = identity_changes;
        }
        state.champion_key = Some(champion_key.to_string());
        state.last_entity_id = Some(entity_id);
    }
    entity_owners()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert((sim_id, entity_id), player_id);
}

/// Binds a champion entity to its owning player in the runtime owner map.
/// Called by the match driver each tick so `player_for_entity` keeps resolving
/// even when a native effect apply cannot enumerate players, and so the new
/// champion entity is tracked the moment a player respawns. No identity or
/// state is reset here; that is handled by `register_entity` / `init`.
pub fn bind_entity(sim: &StableSim<'_>, player_id: usize, entity_id: usize) {
    entity_owners()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert((sim_instance_id(sim), entity_id), player_id);
}

pub fn clear_player(sim: &StableSim<'_>, player_id: usize) {
    let mut guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.remove(&player_key(sim, player_id));
    let sim_id = sim_instance_id(sim);
    entity_owners()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .retain(|(stored_sim_id, _), owner| *stored_sim_id != sim_id || *owner != player_id);
    spawned_units_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .retain(|(stored_sim_id, unit)| *stored_sim_id != sim_id || unit.owner_id != player_id);
    cancelled_spawned_unit_effects()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .retain(|(stored_sim_id, stored_owner_id, _)| {
            *stored_sim_id != sim_id || *stored_owner_id != player_id
        });
}

/// Clears only runtime bookkeeping owned by the supplied simulation. Other
/// simulations running concurrently in the same DLL are intentionally kept.
pub fn clear_sim(sim: &StableSim<'_>) {
    let sim_id = sim_instance_id(sim);
    {
        let mut guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.retain(|(stored_sim_id, _), _| *stored_sim_id != sim_id);
    }
    {
        let mut guard = periodics().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.retain(|instance| instance.sim_id != sim_id);
    }
    {
        let mut guard = entity_owners().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.retain(|(stored_sim_id, _), _| *stored_sim_id != sim_id);
    }
    spawned_units_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .retain(|(stored_sim_id, _)| *stored_sim_id != sim_id);
    cancelled_spawned_unit_effects()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .retain(|(stored_sim_id, _, _)| *stored_sim_id != sim_id);
    spawn_aggregation_scopes()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&sim_id);
    if let Some(store) = BUFF_VFX_BY_SIM.get() {
        store.lock().unwrap_or_else(|p| p.into_inner()).remove(&sim_id);
    }
    if let Some(starts) = BUFF_VFX_STARTS.get() {
        starts.lock().unwrap_or_else(|p| p.into_inner()).retain(|(sid, _, _), _| *sid != sim_id);
    }
    MISSED_OWNER_LOOKUPS.store(0, Ordering::Relaxed);
}

pub fn track_spawned_unit(
    sim: &StableSim<'_>,
    owner_id: usize,
    entity_id: usize,
    name: &str,
    attack_enabled: bool,
) -> u64 {
    let sim_id = sim_instance_id(sim);
    let serial = NEXT_SPAWNED_UNIT_SERIAL.fetch_add(1, Ordering::Relaxed);
    let unit = SpawnedUnit {
        serial,
        owner_id,
        entity_id,
        name: name.to_string(),
        created_tick: sim.tick() as u64,
        attack_enabled,
        attack_gate_queued: false,
    };
    spawned_units_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push((sim_id, unit));
    cancelled_spawned_unit_effects()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .retain(|(stored_sim_id, stored_owner_id, stored_entity_id)| {
            !(*stored_sim_id == sim_id
                && *stored_owner_id == owner_id
                && *stored_entity_id == entity_id)
        });
    serial
}

pub fn set_spawn_attack_enabled(sim: &mut StableSim<'_>, entity_id: usize, enabled: bool) {
    let sim_id = sim_instance_id(sim);
    let mut should_queue = false;
    let mut owner_id = entity_id;
    {
        let mut guard = spawned_units_store().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some((_, unit)) = guard.iter_mut().find(|(stored_sim_id, unit)| *stored_sim_id == sim_id && unit.entity_id == entity_id) {
            unit.attack_enabled = enabled;
            owner_id = unit.owner_id;
            if !enabled && !unit.attack_gate_queued {
                unit.attack_gate_queued = true;
                should_queue = true;
            }
        }
    }
    if !enabled {
        let gate = CcV1::of_kind(CcKindV1::BlockAttack, 2);
        sim.apply_cc(entity_id, &gate);
        if should_queue {
            let _ = sim.queue_effect("__forge_spawn_attack_gate", AttackTypeV1::Skill, owner_id, &InputTargetV1::target(entity_id), 1);
        }
    }
}

pub fn maintain_spawn_attack_gate(sim: &mut StableSim<'_>, owner_id: usize, entity_id: usize) {
    let sim_id = sim_instance_id(sim);
    let should_continue = {
        let mut guard = spawned_units_store().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some((_, unit)) = guard.iter_mut().find(|(stored_sim_id, unit)| *stored_sim_id == sim_id && unit.owner_id == owner_id && unit.entity_id == entity_id) else { return; };
        if unit.attack_enabled || sim.get_entity(entity_id).map(|entity| !entity.is_alive()).unwrap_or(true) {
            unit.attack_gate_queued = false;
            false
        } else {
            true
        }
    };
    if should_continue {
        let gate = CcV1::of_kind(CcKindV1::BlockAttack, 2);
        sim.apply_cc(entity_id, &gate);
        let _ = sim.queue_effect("__forge_spawn_attack_gate", AttackTypeV1::Skill, owner_id, &InputTargetV1::target(entity_id), 1);
    }
}

pub fn begin_spawn_aggregation(sim: &StableSim<'_>, allow_duplicates: bool, additional_multiplier: f32) {
    spawn_aggregation_scopes().lock().unwrap_or_else(|poisoned| poisoned.into_inner())
        .entry(sim_instance_id(sim)).or_default().push(SpawnAggregationScope {
            allow_duplicates,
            additional_multiplier: additional_multiplier.max(0.0),
            ..Default::default()
        });
}

pub fn aggregate_spawn_targets(sim: &StableSim<'_>, targets: Vec<usize>) -> Vec<usize> {
    let mut guard = spawn_aggregation_scopes().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(scope) = guard.get_mut(&sim_instance_id(sim)).and_then(|scopes| scopes.last_mut()) else { return targets; };
    scope.current_multipliers.clear();
    targets.into_iter().filter(|target_id| {
        let count = scope.hit_counts.entry(*target_id).or_insert(0);
        *count += 1;
        let first = *count == 1;
        scope.current_multipliers.insert(*target_id, if first { 1.0 } else { scope.additional_multiplier });
        first || scope.allow_duplicates
    }).collect()
}

pub fn spawn_effect_multiplier(sim: &StableSim<'_>, target_id: usize) -> f32 {
    spawn_aggregation_scopes().lock().unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&sim_instance_id(sim)).and_then(|scopes| scopes.last())
        .and_then(|scope| scope.current_multipliers.get(&target_id).copied()).unwrap_or(1.0)
}

pub fn end_spawn_aggregation(sim: &StableSim<'_>) {
    let sim_id = sim_instance_id(sim);
    let mut guard = spawn_aggregation_scopes().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(scopes) = guard.get_mut(&sim_id) {
        scopes.pop();
        if scopes.is_empty() { guard.remove(&sim_id); }
    }
}

pub fn spawned_units(
    sim: &StableSim<'_>,
    owner_id: usize,
    name: &str,
) -> Vec<SpawnedUnit> {
    let sim_id = sim_instance_id(sim);
    let mut guard = spawned_units_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.retain(|(stored_sim_id, unit)| {
        *stored_sim_id != sim_id
            || (sim
                .get_entity(unit.entity_id)
                .map(|entity| entity.is_alive())
                .unwrap_or(false))
    });
    guard
        .iter()
        .filter(|(stored_sim_id, unit)| {
            *stored_sim_id == sim_id && unit.owner_id == owner_id && unit.name == name
        })
        .map(|(_, unit)| unit.clone())
        .collect()
}

pub fn spawned_unit_ids(sim: &StableSim<'_>, owner_id: usize, name: &str) -> Vec<usize> {
    spawned_units(sim, owner_id, name)
        .into_iter()
        .map(|unit| unit.entity_id)
        .collect()
}

pub fn forget_spawned_unit(sim: &StableSim<'_>, serial: u64) {
    let sim_id = sim_instance_id(sim);
    spawned_units_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .retain(|(stored_sim_id, unit)| *stored_sim_id != sim_id || unit.serial != serial);
}

pub fn spawned_unit_effect_is_active(
    sim: &StableSim<'_>,
    owner_id: usize,
    entity_id: usize,
) -> bool {
    let sim_id = sim_instance_id(sim);
    let cancelled = cancelled_spawned_unit_effects()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .any(|(stored_sim_id, stored_owner_id, stored_entity_id)| {
            *stored_sim_id == sim_id
                && *stored_owner_id == owner_id
                && *stored_entity_id == entity_id
        });
    if cancelled {
        return false;
    }
    let tracked_unit = spawned_units_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .find(|(stored_sim_id, unit)| {
            *stored_sim_id == sim_id
                && unit.owner_id == owner_id
                && unit.entity_id == entity_id
        })
        .map(|(_, unit)| unit.clone());
    let Some(tracked_unit) = tracked_unit else {
        return false;
    };
    sim.get_entity(entity_id)
        .map(|entity| entity.is_alive())
        .unwrap_or(tracked_unit.created_tick == sim.tick() as u64)
}

pub fn queue_effect_if_active(
    sim: &mut StableSim<'_>,
    effect_name: &str,
    attack_type: AttackTypeV1,
    caster_id: usize,
    target_id: usize,
    delay_ticks: usize,
) -> bool {
    if !spawned_unit_effect_is_active(sim, caster_id, target_id) {
        return false;
    }
    sim.queue_effect(
        effect_name,
        attack_type,
        caster_id,
        &InputTargetV1::target(target_id),
        delay_ticks,
    )
}

pub fn cancel_spawned_unit_effect(
    sim: &mut StableSim<'_>,
    owner_id: usize,
    entity_id: usize,
    destroy_spawn_on_trigger: bool,
) {
    let sim_id = sim_instance_id(sim);
    let mut cancelled = cancelled_spawned_unit_effects()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !cancelled.iter().any(|(stored_sim_id, stored_owner_id, stored_entity_id)| {
        *stored_sim_id == sim_id
            && *stored_owner_id == owner_id
            && *stored_entity_id == entity_id
    }) {
        cancelled.push((sim_id, owner_id, entity_id));
    }
    drop(cancelled);
    if destroy_spawn_on_trigger {
        let _ = sim.entity_set_hp(entity_id, 0);
        spawned_units_store()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retain(|(stored_sim_id, unit)| {
                !(*stored_sim_id == sim_id
                    && unit.owner_id == owner_id
                    && unit.entity_id == entity_id)
            });
    }
}

pub fn consume_spawned_unit_target(
    sim: &mut StableSim<'_>,
    owner_id: usize,
    entity_id: usize,
    destroy_spawn_on_trigger: bool,
) {
    cancel_spawned_unit_effect(sim, owner_id, entity_id, destroy_spawn_on_trigger);
}

fn player_for_entity(sim: &StableSim<'_>, entity_id: usize) -> Option<usize> {
    for index in 0..sim.player_count() {
        let Some(player) = sim.player_at(index) else { continue; };
        if player.champion().map(|champion| champion.id()) == Some(entity_id) {
            return Some(player.id());
        }
    }
    if let Some(player_id) = entity_owners()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&(sim_instance_id(sim), entity_id))
        .copied()
    {
        return Some(player_id);
    }
    let owner_entity_id = spawned_units_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .find(|(stored_sim_id, unit)| {
            *stored_sim_id == sim_instance_id(sim) && unit.entity_id == entity_id
        })
        .map(|(_, unit)| unit.owner_id);
    owner_entity_id
        .filter(|owner_id| *owner_id != entity_id)
        .and_then(|owner_id| player_for_entity(sim, owner_id))
}

pub fn add_gold_for_entity(sim: &mut StableSim<'_>, entity_id: usize, delta: i64) -> bool {
    player_for_entity(sim, entity_id)
        .map(|player_id| sim.player_add_gold(player_id, delta))
        .unwrap_or(false)
}

pub fn level_for_entity(sim: &StableSim<'_>, entity_id: usize) -> usize {
    player_for_entity(sim, entity_id)
        .and_then(|player_id| sim.get_player(player_id))
        .map(|player| player.level())
        .unwrap_or(1)
}

pub fn add_counter(sim: &StableSim<'_>, entity_id: usize, name: &str, delta: i64) {
    let Some(player_id) = player_for_entity(sim, entity_id) else {
        crate::diagnostic_log!(&format!(
            "COUNTER_ADD_MISSED sim={} tick={} entity={} name={} delta={}",
            sim_instance_id(sim), sim.tick(), entity_id, name, delta
        ));
        return;
    };
    let mut guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let state = guard.entry(player_key(sim, player_id)).or_default();
    let value = state
        .persistent_counters
        .entry(name.to_string())
        .or_insert(0);
    let before = *value;
    *value = value.saturating_add(delta).max(0);
    crate::diagnostic_log!(&format!(
        "COUNTER_STATE sim={} tick={} player={} entity={} name={} before={} delta={} after={}",
        sim_instance_id(sim), sim.tick(), player_id, entity_id, name, before, delta, *value
    ));
}

/// Applies a persistent ModifyStats stack delta to the stored absolute stack.
/// The stored value is authoritative once the first persistent application has
/// been captured, so a respawn or other live stat reset cannot make a later
/// `+3` application become `current_stack + 3` instead of `persisted + 3`.
pub fn next_persistent_modify_stats_stack(
    sim: &StableSim<'_>,
    entity_id: usize,
    current_stack: usize,
    delta: i64,
) -> usize {
    let Some(player_id) = player_for_entity(sim, entity_id) else {
        return (current_stack as i64).saturating_add(delta).max(0) as usize;
    };
    let mut guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let state = guard.entry(player_key(sim, player_id)).or_default();
    let base = state
        .persistent_modify_stats_stack
        .unwrap_or(current_stack) as i64;
    let next = base.saturating_add(delta).max(0) as usize;
    state.persistent_modify_stats_stack = Some(next);
    next
}

pub fn reset_counter(sim: &StableSim<'_>, entity_id: usize, name: &str) {
    let Some(player_id) = player_for_entity(sim, entity_id) else {
        crate::diagnostic_log!(&format!(
            "COUNTER_RESET_MISSED sim={} tick={} entity={} name={}",
            sim_instance_id(sim), sim.tick(), entity_id, name
        ));
        return;
    };
    let mut guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let before = guard
        .get(&player_key(sim, player_id))
        .and_then(|state| state.persistent_counters.get(name))
        .copied()
        .unwrap_or(0);
    if let Some(state) = guard.get_mut(&player_key(sim, player_id)) {
        state.persistent_counters.remove(name);
    }
    crate::diagnostic_log!(&format!(
        "COUNTER_RESET sim={} tick={} player={} entity={} name={} before={} after=0",
        sim_instance_id(sim), sim.tick(), player_id, entity_id, name, before
    ));
}

pub fn set_flag(sim: &StableSim<'_>, entity_id: usize, name: &str, value: bool) {
    let Some(player_id) = player_for_entity(sim, entity_id) else { return; };
    let mut guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .entry(player_key(sim, player_id))
        .or_default()
        .flags
        .insert(name.to_string(), value);
}

pub fn counter_at_least(sim: &StableSim<'_>, entity_id: usize, name: &str, count: usize) -> bool {
    let Some(player_id) = player_for_entity(sim, entity_id) else { return false; };
    let guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .get(&player_key(sim, player_id))
        .and_then(|state| state.persistent_counters.get(name))
        .copied()
        .unwrap_or(0)
        >= count as i64
}

/// Raw current value of a named counter (0 if never set). Unlike
/// `counter_at_least`, this returns the actual stored number rather than a
/// threshold check — used by Saitama's combo-tier scaling.
pub fn counter_value(sim: &StableSim<'_>, entity_id: usize, name: &str) -> i64 {
    let Some(player_id) = player_for_entity(sim, entity_id) else { return 0; };
    let guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .get(&player_key(sim, player_id))
        .and_then(|state| state.persistent_counters.get(name))
        .copied()
        .unwrap_or(0)
}

/// Scans the sim's kill log (the same `kill_log_count`/`kill_log_at` source
/// `DealDamage`'s `trigger_effect_on_kill` reads from) for entries crediting
/// this entity's player with a kill or an assist since the last time this
/// counter was checked, and returns how many new credited entries were
/// found. The kill log identifies participants by team + lane (not entity
/// id), so this is a best-effort match: it cannot distinguish two players on
/// the same team sharing a lane. Callers grant one stack of a buff per
/// returned count.
pub fn takedown_stack_delta(sim: &StableSim<'_>, entity_id: usize, counter_name: &str) -> u32 {
    let Some(player_id) = player_for_entity(sim, entity_id) else { return 0; };
    let Some(player) = sim.get_player(player_id) else { return 0; };
    let Some(lane) = player.lane() else { return 0; };
    let team = player.team();
    let index_key = format!("{counter_name}__kill_log_index");
    let last_index = {
        let guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard
            .get(&player_key(sim, player_id))
            .and_then(|state| state.persistent_counters.get(&index_key))
            .copied()
            .unwrap_or(0)
            .max(0) as usize
    };
    let total = sim.kill_log_count();
    if total <= last_index {
        return 0;
    }
    let mut stacks_gained = 0u32;
    for index in last_index..total {
        let Some(entry) = sim.kill_log_at(index) else { continue; };
        if entry.killer_team != team {
            continue;
        }
        let lane_code = lane.code();
        let credited = entry.killer_position == lane_code
            || (0..entry.assist_count as usize)
                .any(|i| entry.assist_positions[i] == lane_code);
        if credited {
            stacks_gained += 1;
        }
    }
    {
        let mut guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard
            .entry(player_key(sim, player_id))
            .or_default()
            .persistent_counters
            .insert(index_key, total as i64);
    }
    stacks_gained
}

pub fn flag_is_set(sim: &StableSim<'_>, entity_id: usize, name: &str) -> bool {
    let Some(player_id) = player_for_entity(sim, entity_id) else { return false; };
    let guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .get(&player_key(sim, player_id))
        .and_then(|state| state.flags.get(name))
        .copied()
        .unwrap_or(false)
}

/// Remembers the latest base-stat snapshot produced by a persistent
/// ModifyStats effect. The snapshot is keyed by player rather than entity so
/// it survives the champion entity being destroyed/replaced on death.
pub fn remember_base_stat(sim: &StableSim<'_>, entity_id: usize, stat: &StatV1) {
    let Some(player_id) = player_for_entity(sim, entity_id) else { return; };
    let mut guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let state = guard.entry(player_key(sim, player_id)).or_default();
    state.persistent_base_stat = Some(*stat);
    state.persistent_modify_stats_stack = Some(stat.stack);
    state.last_entity_id = Some(entity_id);
    state.was_alive = sim
        .get_entity(entity_id)
        .map(|entity| entity.is_alive())
        .unwrap_or(false);
    state.entity_missing = false;
    state.persistent_capture_count = state.persistent_capture_count.saturating_add(1);
}

fn restore_persistent_base_stat(sim: &mut StableSim<'_>, player_id: usize, entity_id: usize) {
    let stat = {
        let guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard
            .get(&player_key(sim, player_id))
            .and_then(|state| state.persistent_base_stat)
    };
    if let Some(stat) = stat {
        let _ = sim.entity_set_base_stat(entity_id, &stat);
    }
}

pub fn refresh_persistent_state(sim: &mut StableSim<'_>, entity_id: usize) {
    let Some(player_id) = player_for_entity(sim, entity_id) else { return; };
    restore_persistent_buffs(sim, player_id, entity_id);
    let current_stack = sim
        .get_entity(entity_id)
        .map(|entity| entity.stat().stack)
        .unwrap_or(0);
    let persisted_stack = {
        let guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard
            .get(&player_key(sim, player_id))
            .and_then(|state| state.persistent_modify_stats_stack)
    };
    if persisted_stack.is_some_and(|stack| current_stack < stack) {
        restore_persistent_base_stat(sim, player_id, entity_id);
    }
}

/// Applies a buff normally and, when requested, remembers an identical copy
/// against the champion owner's player id so it can be restored after death.
/// Non-player entities simply receive the normal buff application.
pub fn add_buff(
    sim: &mut StableSim<'_>,
    target_id: usize,
    buff: &BuffV1,
    persist_through_death: bool,
    stackable: bool,
    max_count: usize,
    refresh_stacks: bool,
) {
    let player_id = if persist_through_death {
        player_for_entity(sim, target_id)
    } else {
        None
    };
    if persist_through_death && player_id.is_none() {
        MISSED_OWNER_LOOKUPS.fetch_add(1, Ordering::Relaxed);
    }
    let target_alive = sim
        .get_entity(target_id)
        .map(|entity| entity.is_alive())
        .unwrap_or(false);
    let now = sim.tick();

    let effective_max = if stackable { max_count } else { 1 };
    if stackable {
        // Counts same-name stat buffs, optionally refreshes their remaining
        // duration, and appends one more copy under the max_stacks cap -
        // replacing the read-remove-re-add loop this used to hand-roll.
        let _ = sim.entity_stack_buff(target_id, buff, max_count, refresh_stacks);
    } else {
        let live_count = sim.get_entity(target_id).map(|entity| {
            (0..entity.buff_count())
                .filter(|index| entity.buff_at(*index).map(|current| current.name() == buff.name()).unwrap_or(false))
                .count()
        }).unwrap_or(0);
        if live_count > 0 {
            // BuffV1 exposes no in-place duration mutation, so a timed buff
            // at its cap of 1 is refreshed by recreating it; a permanent buff
            // already applied has nothing to refresh.
            if buff.duration_kind == BuffDurationV1::Time.code() {
                let _ = sim.entity_remove_buff(target_id, buff.name());
                sim.add_buff(target_id, buff);
            }
        } else {
            sim.add_buff(target_id, buff);
        }
    }

    let Some(player_id) = player_id else { return; };
    let expires_at = if buff.duration_kind == BuffDurationV1::Time.code() {
        Some(now.saturating_add(buff.duration_tick))
    } else {
        None
    };
    let mut guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let state = guard.entry(player_key(sim, player_id)).or_default();
    state
        .persistent_buffs
        .retain(|stored| stored.expires_at.map_or(true, |stored_expiry| stored_expiry > now));
    let persisted_count = state
        .persistent_buffs
        .iter()
        .filter(|stored| stored.buff.name() == buff.name())
        .count();
    let persisted_at_cap = effective_max != 0 && persisted_count >= effective_max;
    if persisted_at_cap {
        let mut kept = 0usize;
        state.persistent_buffs.retain_mut(|stored| {
            if stored.buff.name() != buff.name() {
                return true;
            }
            if kept >= effective_max {
                return false;
            }
            stored.buff = *buff;
            stored.expires_at = expires_at;
            kept = kept.saturating_add(1);
            true
        });
    } else {
        state.persistent_buffs.push(PersistentBuff {
            buff: *buff,
            expires_at,
        });
    }
    state.last_entity_id = Some(target_id);
    state.was_alive = target_alive;
    state.entity_missing = false;
    state.persistent_capture_count = state.persistent_capture_count.saturating_add(1);
    drop(guard);
    // The persisted copies are the source of truth for the stack count. If the
    // target is missing stacks (e.g. it respawned since the last application),
    // restore the shortfall now so the very next application heals the full
    // stack back.
    reconcile_buff_stacks(sim, target_id, player_id, buff.name());
}

/// Removes the live buff and forgets all persistent applications with the
/// same name, preventing an explicitly removed buff from returning on respawn.
pub fn remove_buff(sim: &mut StableSim<'_>, target_id: usize, name: &str) -> usize {
    let removed = sim.entity_remove_buff(target_id, name);
    if let Some(player_id) = player_for_entity(sim, target_id) {
        let mut guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(state) = guard.get_mut(&player_key(sim, player_id)) {
            state.persistent_buffs.retain(|stored| stored.buff.name() != name);
        }
    }
    removed
}

/// Changes the remaining lifetime of every live timed stack with this name.
/// The complete BuffV1 values are copied before removal so all stats and stack
/// copies survive. Durations reduced to zero are intentionally not reapplied.
pub fn modify_internal_cooldown(
    sim: &mut StableSim<'_>,
    target_id: usize,
    name: &str,
    delta_ticks: i32,
) {
    let copies: Vec<BuffV1> = sim.get_entity(target_id).map(|entity| {
        (0..entity.buff_count())
            .filter_map(|index| entity.buff_at(index))
            .filter(|buff| buff.name() == name && buff.duration_kind == BuffDurationV1::Time.code())
            .collect()
    }).unwrap_or_default();
    if copies.is_empty() {
        return;
    }

    let _ = sim.entity_remove_buff(target_id, name);
    for mut buff in copies {
        let next = if delta_ticks >= 0 {
            buff.duration_tick.saturating_add(delta_ticks as usize)
        } else {
            buff.duration_tick.saturating_sub(delta_ticks.unsigned_abs() as usize)
        };
        if next > 0 {
            buff.duration_tick = next;
            sim.add_buff(target_id, &buff);
        }
    }
}

/// Makes the target's live stack count for `name` match the persisted count,
/// re-adding the missing copies with their remaining durations. Idempotent:
/// only the shortfall is applied, so overlapping restore and re-application
/// paths never double-add. Persisted copies whose time window already passed
/// are pruned first.
fn reconcile_buff_stacks(sim: &mut StableSim<'_>, target_id: usize, player_id: usize, name: &str) {
    let now = sim.tick();
    let copies = {
        let mut guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(state) = guard.get_mut(&player_key(sim, player_id)) else { return; };
        state
            .persistent_buffs
            .retain(|stored| stored.expires_at.map_or(true, |expires_at| expires_at > now));
        state
            .persistent_buffs
            .iter()
            .filter(|stored| stored.buff.name() == name)
            .cloned()
            .collect::<Vec<_>>()
    };

    if copies.is_empty() {
        return;
    }

    let mut live_count = 0usize;
    if let Some(entity) = sim.get_entity(target_id) {
        for index in 0..entity.buff_count() {
            if let Some(buff) = entity.buff_at(index) {
                if buff.name() == name {
                    live_count += 1;
                }
            }
        }
    }

    let missing = copies.len().saturating_sub(live_count);
    if missing == 0 {
        return;
    }
    let mut restored_count = 0usize;
    for stored in copies.iter().skip(live_count) {
        let mut buff = stored.buff;
        if let Some(expires_at) = stored.expires_at {
            buff.duration_kind = BuffDurationV1::Time.code();
            buff.duration_tick = expires_at.saturating_sub(now);
        }
        sim.add_buff(target_id, &buff);
        restored_count += 1;
    }
    if restored_count > 0 {
        let mut guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(state) = guard.get_mut(&player_key(sim, player_id)) {
            state.restored_buff_count = state.restored_buff_count.saturating_add(restored_count);
        }
    }
}

fn restore_persistent_buffs(sim: &mut StableSim<'_>, player_id: usize, entity_id: usize) {
    let names = {
        let mut guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(state) = guard.get_mut(&player_key(sim, player_id)) else { return; };
        let now = sim.tick();
        state
            .persistent_buffs
            .retain(|stored| stored.expires_at.map_or(true, |expires_at| expires_at > now));
        let mut names = Vec::new();
        for stored in &state.persistent_buffs {
            let name = stored.buff.name().to_string();
            if !names.contains(&name) {
                names.push(name);
            }
        }
        names
    };

    for name in names {
        reconcile_buff_stacks(sim, entity_id, player_id, &name);
    }
}

/// Called by the generated match driver each tick. A dead -> alive transition
/// (or an entity-id replacement while alive) restores the persistent base
/// stat; while alive, persisted stacks are topped up every tick so buffs
/// registered with `persist_through_death` come back after respawn even if the
/// transition is missed or the game clears buffs a tick later.
pub fn update_player_lifecycle(
    sim: &mut StableSim<'_>,
    player_id: usize,
    entity_id: Option<usize>,
    alive: bool,
) {
    let Some(entity_id) = entity_id else {
        // Some hosts temporarily expose no champion entity while the player is
        // dead/respawning. Remember the gap even if the host still reports the
        // player as alive; the next entity must be treated as a respawn.
        let mut guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let state = guard.entry(player_key(sim, player_id)).or_default();
        state.was_alive = alive;
        state.entity_missing = true;
        return;
    };

    let mut guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let state = guard.entry(player_key(sim, player_id)).or_default();
    state.was_alive = alive;
    state.last_entity_id = Some(entity_id);
    state.entity_missing = false;
    drop(guard);
    // Top up any persisted stacks the target is missing every live tick.
    // Idempotent (only the shortfall is applied), so this self-heals respawns
    // even when the transition above fired before the new entity was ready or
    // the game cleared its buffs a tick later.
    if alive {
        restore_persistent_buffs(sim, player_id, entity_id);
    }
}

/// Returns persistence counters for diagnostics: captures, restore events,
/// restored buff applications, missed owner lookups, and champion identity
/// changes.
pub fn persistence_diagnostics(
    sim: &StableSim<'_>,
    player_id: usize,
) -> (usize, usize, usize, usize, usize) {
    let guard = states().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(state) = guard.get(&player_key(sim, player_id)) else {
        return (0, 0, 0, MISSED_OWNER_LOOKUPS.load(Ordering::Relaxed), 0);
    };
    (
        state.persistent_capture_count,
        state.restore_count,
        state.restored_buff_count,
        MISSED_OWNER_LOOKUPS.load(Ordering::Relaxed),
        state.identity_change_count,
    )
}

#[derive(Clone)]
pub struct PeriodicInstance {
    sim_id: SimId,
    pub source_id: usize,
    pub target_id: usize,
    pub effect_id: u64,
    pub end_tick: u64,
    pub next_tick: u64,
    pub interval: u64,
    pub stacks: usize,
}

static PERIODICS: OnceLock<Mutex<Vec<PeriodicInstance>>> = OnceLock::new();

fn periodics() -> &'static Mutex<Vec<PeriodicInstance>> {
    PERIODICS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn register_periodic(
    sim: &StableSim<'_>,
    source_id: usize,
    target_id: usize,
    effect_id: u64,
    duration: u64,
    interval: u64,
    first_instant: bool,
    can_stack: bool,
    max_stacks: usize,
    refresh: bool,
) {
    if duration == 0 || sim.get_entity(target_id).is_none() { return; }
    let tick = sim.tick() as u64;
    let sim_id = sim_instance_id(sim);
    let mut guard = periodics().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(instance) = guard.iter_mut().find(|instance| instance.sim_id == sim_id && instance.source_id == source_id && instance.target_id == target_id && instance.effect_id == effect_id) {
        if can_stack {
            if max_stacks == 0 || instance.stacks < max_stacks { instance.stacks = instance.stacks.saturating_add(1); }
        }
        if refresh { instance.end_tick = tick.saturating_add(duration); }
        return;
    }
    guard.push(PeriodicInstance {
        sim_id: sim_instance_id(sim),
        source_id,
        target_id,
        effect_id,
        end_tick: tick.saturating_add(duration),
        next_tick: if first_instant { tick } else { tick.saturating_add(interval.max(1)) },
        interval: interval.max(1),
        stacks: 1,
    });
}

pub fn register_delayed(sim: &StableSim<'_>, source_id: usize, target_id: usize, effect_id: u64, delay: u64) {
    register_periodic(sim, source_id, target_id, effect_id, delay.saturating_add(1), delay.max(1), false, false, 1, false);
}

pub fn due_periodics(sim: &StableSim<'_>, source_id: usize, tick: u64) -> Vec<PeriodicInstance> {
    let sim_id = sim_instance_id(sim);
    let mut guard = periodics().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    // Only inspect entity ids belonging to this simulation. Entity ids from a
    // concurrent sim are meaningless in the current StableSim.
    guard.retain(|instance| {
        instance.sim_id != sim_id
            || (instance.end_tick > tick
                && sim
                    .get_entity(instance.target_id)
                    .map(|entity| entity.is_alive())
                    .unwrap_or(false))
    });
    guard
        .iter()
        .filter(|instance| {
            instance.sim_id == sim_id
                && instance.source_id == source_id
                && instance.next_tick <= tick
        })
        .cloned()
        .collect()
}

pub fn advance_periodic(sim: &StableSim<'_>, source_id: usize, target_id: usize, effect_id: u64) {
    let sim_id = sim_instance_id(sim);
    let mut guard = periodics().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(instance) = guard.iter_mut().find(|instance| instance.sim_id == sim_id && instance.source_id == source_id && instance.target_id == target_id && instance.effect_id == effect_id) {
        instance.next_tick = instance.next_tick.saturating_add(instance.interval);
    }
}

pub fn clear_periodics_for_source(sim: &StableSim<'_>, source_id: usize) {
    let sim_id = sim_instance_id(sim);
    let mut guard = periodics().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.retain(|instance| instance.sim_id != sim_id || instance.source_id != source_id);
}

pub fn clear_periodics(sim: &StableSim<'_>) {
    let sim_id = sim_instance_id(sim);
    let mut guard = periodics().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.retain(|instance| instance.sim_id != sim_id);
}

#[derive(Clone)]
pub struct BuffVfxSnapshot {
    pub binding_name: String,
    pub entity_id: usize,
    pub x: f32,
    pub y: f32,
    pub elapsed_ticks: usize,
}

static BUFF_VFX_BY_SIM: OnceLock<Mutex<HashMap<SimId, Vec<BuffVfxSnapshot>>>> = OnceLock::new();
static BUFF_VFX_STARTS: OnceLock<Mutex<HashMap<(SimId, usize, String), usize>>> = OnceLock::new();
static LATEST_BUFF_VFX_SIM: AtomicUsize = AtomicUsize::new(0);

fn buff_vfx_by_sim() -> &'static Mutex<HashMap<SimId, Vec<BuffVfxSnapshot>>> {
    BUFF_VFX_BY_SIM.get_or_init(|| Mutex::new(HashMap::new()))
}

fn buff_vfx_starts() -> &'static Mutex<HashMap<(SimId, usize, String), usize>> {
    BUFF_VFX_STARTS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn refresh_buff_visuals(sim: &StableSim<'_>) {
    let sim_id = sim_instance_id(sim);
    let now = sim.tick();
    let mut current = HashSet::<(usize, String)>::new();
    let mut snapshots = Vec::new();
    let mut starts = buff_vfx_starts().lock().unwrap_or_else(|p| p.into_inner());

    for index in 0..sim.entity_count() {
        let Some(entity) = sim.entity_at(index) else { continue; };
        if !entity.is_alive() { continue; }
        let entity_id = entity.id();
        let pos = entity.pos();
        let mut names = HashSet::<String>::new();
        for buff_index in 0..entity.buff_count() {
            let Some(buff) = entity.buff_at(buff_index) else { continue; };
            let name = buff.name();
            if !crate::buff_visuals::binding_exists(name) || !names.insert(name.to_string()) {
                continue;
            }
            let key = (sim_id, entity_id, name.to_string());
            let started = *starts.entry(key).or_insert(now);
            current.insert((entity_id, name.to_string()));
            snapshots.push(BuffVfxSnapshot {
                binding_name: name.to_string(),
                entity_id,
                x: pos.0 as f32 / 1000.0,
                y: pos.1 as f32 / 1000.0,
                elapsed_ticks: now.saturating_sub(started),
            });
        }
    }

    starts.retain(|(sid, entity_id, name), _| {
        *sid != sim_id || current.contains(&(*entity_id, name.clone()))
    });
    drop(starts);

    buff_vfx_by_sim()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(sim_id, snapshots);
    // The client SDK does not expose the rendered SimId. Keep snapshots isolated
    // and publish only one complete simulation at a time as the current candidate.
    LATEST_BUFF_VFX_SIM.store(sim_id, Ordering::Release);
}

pub fn buff_vfx_snapshot() -> Vec<BuffVfxSnapshot> {
    let sim_id = LATEST_BUFF_VFX_SIM.load(Ordering::Acquire);
    buff_vfx_by_sim()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&sim_id)
        .cloned()
        .unwrap_or_default()
}

struct ProjectileViewEffect {
    sim_id: SimId,
    caster_id: usize,
    name: String,
    effect_name: String,
    radius: u64,
    last_x: u64,
    last_y: u64,
}

static PROJECTILE_VIEW_EFFECTS: OnceLock<Mutex<HashMap<usize, ProjectileViewEffect>>> = OnceLock::new();
static NEXT_PROJECTILE_VIEW_EFFECT_ID: AtomicUsize = AtomicUsize::new(1);

fn projectile_view_effects() -> &'static Mutex<HashMap<usize, ProjectileViewEffect>> {
    PROJECTILE_VIEW_EFFECTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Presentation-only work (tracking a view effect along a projectile's path)
/// should never run in background server pre-simulation, which replays the
/// same ability code many times over for odds calculation with nothing on
/// screen. Only a sim actually being watched live is worth the per-tick cost.
fn is_presentation_viable(sim: &StableSim<'_>) -> bool {
    match sim.sim_origin() {
        Some(origin) => {
            origin.kind == SimOriginKindV1::ClientMatchView.code()
                || origin.kind == SimOriginKindV1::ClientSpectate.code()
                || origin.kind == SimOriginKindV1::ClientReplay.code()
        }
        None => false,
    }
}

#[allow(dead_code)]
pub fn start_projectile_view_effect(
    sim: &StableSim<'_>,
    caster_id: usize,
    name: &str,
    x: u64,
    y: u64,
    effect_name: &str,
    radius: u64,
    _tick: usize,
) {
    if effect_name.trim().is_empty() || !is_presentation_viable(sim) {
        return;
    }
    let sim_id = sim_instance_id(sim);
    let mut guard = projectile_view_effects().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    // A caster that fires this same projectile again before the previous cast's
    // vfx timed out (rapid attacks, low cooldowns) would otherwise leave two
    // trackers racing to claim the same live projectile by nearest-position
    // guesswork (the SDK exposes no stable per-projectile id to match on
    // instead). Superseding the earlier tracker keeps that guesswork limited
    // to one candidate at a time.
    guard.retain(|_, vfx| vfx.sim_id != sim_id || vfx.caster_id != caster_id || vfx.name != name);
    let launch_id = NEXT_PROJECTILE_VIEW_EFFECT_ID.fetch_add(1, Ordering::Relaxed);
    guard.insert(launch_id, ProjectileViewEffect {
        sim_id, caster_id, name: name.to_string(), effect_name: effect_name.to_string(),
        radius, last_x: x, last_y: y,
    });
}

/// Clears only the tracked view effects owned by the supplied simulation.
/// Other simulations running concurrently in the same DLL (background
/// odds-calculation replays alongside a live spectated match, for example)
/// are intentionally kept — see `clear_sim`.
#[allow(dead_code)]
pub fn clear_projectile_view_effects(sim: &StableSim<'_>) {
    let sim_id = sim_instance_id(sim);
    let mut guard = projectile_view_effects().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.retain(|_, vfx| vfx.sim_id != sim_id);
}

#[allow(dead_code)]
pub fn update_projectile_view_effects(sim: &mut StableSim<'_>) {
    // Presentation-only work; see `is_presentation_viable`. Background sims
    // never insert trackers either, but skipping here too keeps this function
    // from ever attributing another sim's projectiles to a live one — the
    // guard below only matches entries already scoped to `sim`, but there is
    // no reason to pay the per-tick scan cost when nothing is watching.
    if !is_presentation_viable(sim) {
        return;
    }
    let sim_id = sim_instance_id(sim);
    let mut guard = projectile_view_effects().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if guard.is_empty() {
        return;
    }
    let mut ended = Vec::new();
    for (launch_id, vfx) in guard.iter_mut() {
        if vfx.sim_id != sim_id {
            continue;
        }
        let mut best: Option<(u64, ProjectileInfoV1)> = None;
        for index in 0..sim.projectile_count() {
            let Some(info) = sim.projectile_at(index) else { continue; };
            if info.caster_id != vfx.caster_id { continue; }
            let distance = info.x.abs_diff(vfx.last_x).saturating_mul(info.x.abs_diff(vfx.last_x))
                .saturating_add(info.y.abs_diff(vfx.last_y).saturating_mul(info.y.abs_diff(vfx.last_y)));
            if best.as_ref().map_or(true, |(current, _)| distance < *current) { best = Some((distance, info)); }
        }
        match best {
            Some((_, info)) if !info.is_end => {
                vfx.last_x = info.x;
                vfx.last_y = info.y;
                let target = InputTargetV1::pos(vfx.last_x, vfx.last_y);
                let _ = sim.play_view_effect(&vfx.effect_name, vfx.caster_id, &target, 0, vfx.radius, 1);
            }
            _ => ended.push(*launch_id),
        }
    }
    for launch_id in ended { guard.remove(&launch_id); }
}