use mod_api_stable::{
    AttackTypeV1, BuffV1, CastingTargetV1, CastingTypeV1, InputTargetKindV1, InputTargetV1,
    ProjectileMoveKindV1, ProjectileSpawnV1, StableSim,
};

const TRACKER_PRE_PREFIX: &str = "__forge_track_pre:";
const TRACKER_POST_PREFIX: &str = "__forge_track_post:";

pub fn buff_duration(sim: &StableSim<'_>, target_id: usize, buff_name: &str) -> Option<usize> {
    sim.get_entity(target_id).and_then(|entity| {
        (0..entity.buff_count()).find_map(|index| {
            entity.buff_at(index).and_then(|buff| {
                (buff.name() == buff_name).then_some(buff.duration_tick)
            })
        })
    })
}

pub fn activate_damage_tracker(
    sim: &mut StableSim<'_>,
    target_id: usize,
    tracker_name: &str,
    pre_mitigation: bool,
    window_ticks: usize,
) {
    let prefix = if pre_mitigation { TRACKER_PRE_PREFIX } else { TRACKER_POST_PREFIX };
    let name = format!("{prefix}{tracker_name}");
    let _ = sim.entity_remove_buff(target_id, &name);
    sim.add_buff(target_id, &BuffV1::timed(&name, window_ticks));
}

pub fn record_tracked_damage(
    sim: &mut StableSim<'_>,
    target_id: usize,
    damage: usize,
    pre_mitigation: bool,
) {
    if damage == 0 { return; }
    let prefix = if pre_mitigation { TRACKER_PRE_PREFIX } else { TRACKER_POST_PREFIX };
    let trackers: Vec<(String, usize)> = sim.get_entity(target_id).map(|entity| {
        (0..entity.buff_count()).filter_map(|index| entity.buff_at(index))
            .filter(|buff| buff.name().starts_with(prefix))
            .map(|buff| (buff.name().to_string(), buff.duration_tick))
            .collect()
    }).unwrap_or_default();
    for (name, duration) in trackers {
        let replacement_duration = duration.saturating_add(damage);
        let _ = sim.entity_remove_buff(target_id, &name);
        sim.add_buff(target_id, &BuffV1::timed(&name, replacement_duration));
    }
}

pub fn tracked_damage(sim: &StableSim<'_>, target_id: usize, tracker_name: &str) -> usize {
    [TRACKER_PRE_PREFIX, TRACKER_POST_PREFIX].into_iter()
        .filter_map(|prefix| buff_duration(sim, target_id, &format!("{prefix}{tracker_name}")))
        .max()
        .unwrap_or(0)
}

pub fn count_buff_prefix(sim: &StableSim<'_>, target_id: usize, prefix: &str) -> usize {
    sim.get_entity(target_id)
        .map(|entity| {
            (0..entity.buff_count())
                .filter(|index| {
                    entity
                        .buff_at(*index)
                        .map(|buff| buff.name().starts_with(prefix))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

fn has_buff_name(sim: &StableSim<'_>, target_id: usize, name: &str) -> bool {
    sim.get_entity(target_id)
        .map(|entity| {
            (0..entity.buff_count()).any(|index| {
                entity
                    .buff_at(index)
                    .map(|buff| buff.name() == name)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

pub fn add_unique_timed_marker(
    sim: &mut StableSim<'_>,
    target_id: usize,
    prefix: &str,
    duration_ticks: usize,
) {
    let mut index = count_buff_prefix(sim, target_id, prefix);
    let name = loop {
        let candidate: String = format!("{prefix}{index}").chars().take(64).collect();
        if !has_buff_name(sim, target_id, &candidate) {
            break candidate;
        }
        index = index.saturating_add(1);
    };
    sim.add_buff(target_id, &BuffV1::timed(&name, duration_ticks.max(1)));
}

pub fn add_timed_marker(
    sim: &mut StableSim<'_>,
    target_id: usize,
    name: &str,
    duration_ticks: usize,
) {
    let name: String = name.chars().take(64).collect();
    sim.add_buff(target_id, &BuffV1::timed(&name, duration_ticks.max(1)));
}

pub fn projectile_hit_multiplier(base: f32, hit_count: usize) -> f32 {
    base.max(0.0).powi(hit_count.min(i32::MAX as usize) as i32)
}

pub type EntityPos = (u64, u64);

pub fn integer_sqrt(value: u64) -> u64 {
    if value == 0 {
        return 0;
    }
    let mut low = 0u64;
    let mut high = value.saturating_add(1);
    while low.saturating_add(1) < high {
        let mid = low + (high - low) / 2;
        if mid <= value / mid {
            low = mid;
        } else {
            high = mid;
        }
    }
    low
}

pub fn bounded_force_move_speed(speed: u64, ticks: u64, distance: u64) -> u64 {
    if speed == 0 || ticks == 0 || distance == 0 {
        return 0;
    }
    let required = distance
        .saturating_add(ticks.saturating_sub(1))
        .saturating_div(ticks)
        .max(1);
    speed.min(required)
}

pub fn squared_len_i128(x: i128, y: i128) -> i128 {
    x.saturating_mul(x).saturating_add(y.saturating_mul(y))
}

pub fn distance_sq(a: EntityPos, b: EntityPos) -> u64 {
    let dx = a.0 as i128 - b.0 as i128;
    let dy = a.1 as i128 - b.1 as i128;
    squared_len_i128(dx, dy).min(u64::MAX as i128) as u64
}

pub fn distance_to_segment_sq(point: EntityPos, start: EntityPos, end: EntityPos) -> u64 {
    let px = point.0 as i128;
    let py = point.1 as i128;
    let ax = start.0 as i128;
    let ay = start.1 as i128;
    let abx = end.0 as i128 - ax;
    let aby = end.1 as i128 - ay;
    let ab_len_sq = squared_len_i128(abx, aby);
    if ab_len_sq <= 0 {
        return distance_sq(point, start);
    }
    let t_num = ((px - ax) * abx + (py - ay) * aby).clamp(0, ab_len_sq);
    let dx_num = px * ab_len_sq - (ax * ab_len_sq + abx * t_num);
    let dy_num = py * ab_len_sq - (ay * ab_len_sq + aby * t_num);
    let Some(denominator) = ab_len_sq.checked_mul(ab_len_sq) else {
        return distance_sq(point, start);
    };
    squared_len_i128(dx_num, dy_num)
        .checked_div(denominator)
        .unwrap_or(0)
        .min(u64::MAX as i128) as u64
}

pub fn is_inside_cone(
    point: EntityPos,
    start: EntityPos,
    target: EntityPos,
    length: u64,
    width_at_end: u64,
) -> bool {
    let dir_x = target.0 as f64 - start.0 as f64;
    let dir_y = target.1 as f64 - start.1 as f64;
    let dir_len = (dir_x * dir_x + dir_y * dir_y).sqrt();
    if dir_len <= 0.0 {
        return false;
    }
    let point_x = point.0 as f64 - start.0 as f64;
    let point_y = point.1 as f64 - start.1 as f64;
    let projection = (point_x * dir_x + point_y * dir_y) / dir_len;
    if projection <= 0.0 || projection > length as f64 {
        return false;
    }
    let perpendicular = (point_x * dir_y - point_y * dir_x).abs() / dir_len;
    perpendicular <= (width_at_end as f64 / 2.0) * projection / dir_len
}

pub fn is_inside_cast_rectangle(
    point: EntityPos,
    start: EntityPos,
    target: EntityPos,
    length: u64,
    width: u64,
) -> bool {
    let dx = target.0 as f64 - start.0 as f64;
    let dy = target.1 as f64 - start.1 as f64;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= 0.0 {
        return false;
    }
    let end = (
        (start.0 as f64 + dx / len * length as f64).round().max(0.0) as u64,
        (start.1 as f64 + dy / len * length as f64).round().max(0.0) as u64,
    );
    let segment_x = end.0 as f64 - start.0 as f64;
    let segment_y = end.1 as f64 - start.1 as f64;
    let projection = (point.0 as f64 - start.0 as f64) * segment_x
        + (point.1 as f64 - start.1 as f64) * segment_y;
    let segment_len_sq = segment_x * segment_x + segment_y * segment_y;
    projection >= 0.0
        && projection <= segment_len_sq
        && distance_to_segment_sq(point, start, end) <= (width / 2).saturating_mul(width / 2)
}

pub fn is_jungle_camp(sim: &StableSim<'_>, entity_id: usize) -> bool {
    sim.get_entity(entity_id)
        .map(|entity| {
            entity.is_alive()
                && !entity.is_champion()
                && !entity.is_minion()
                && !entity.is_tower()
                && entity.team() == 2
        })
        .unwrap_or(false)
}

pub fn has_buff(sim: &StableSim<'_>, target_id: usize, buff_name: &str) -> bool {
    has_buff_count(sim, target_id, buff_name, 1)
}

pub fn has_buff_count(
    sim: &StableSim<'_>,
    target_id: usize,
    buff_name: &str,
    minimum_count: usize,
) -> bool {
    sim.get_entity(target_id)
        .map(|entity| {
            (0..entity.buff_count())
                .filter(|&i| {
                    entity
                        .buff_at(i)
                        .map(|buff| buff.name() == buff_name)
                        .unwrap_or(false)
                })
                .count()
                >= minimum_count
        })
        .unwrap_or(false)
}

/// Matches an entity's exact name or a spawned instance name with a numeric
/// suffix, such as `unit1` or `unit12`.
pub fn entity_name_matches(sim: &StableSim<'_>, target_id: usize, expected: &str) -> bool {
    let expected = expected.trim();
    if expected.is_empty() {
        return false;
    }
    let Some(actual) = sim.get_entity(target_id).and_then(|entity| entity.name()) else {
        return false;
    };
    entity_name_matches_value(&actual, expected)
}

fn entity_name_matches_value(actual: &str, expected: &str) -> bool {
    if actual == expected {
        return true;
    }
    actual
        .strip_prefix(expected)
        .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FindBuffTeam {
    Enemy,
    Ally,
}

pub fn find_nearest_with_buff(
    sim: &StableSim<'_>,
    caster_id: usize,
    buff_name: &str,
    team: FindBuffTeam,
    max_targets: usize,
) -> Vec<usize> {
    if max_targets == 0 {
        return Vec::new();
    }
    let Some(caster) = sim.get_entity(caster_id) else {
        return Vec::new();
    };
    let caster_team = caster.team();
    let caster_pos = caster.pos();
    let mut matches = Vec::new();
    for index in 0..sim.entity_count() {
        let Some(entity) = sim.entity_at(index) else {
            continue;
        };
        if !entity.is_alive() || !entity.is_targetable() {
            continue;
        }
        let same_team = entity.team() == caster_team;
        let team_matches = match team {
            FindBuffTeam::Enemy => !same_team,
            FindBuffTeam::Ally => same_team,
        };
        if !team_matches || !has_buff(sim, entity.id(), buff_name) {
            continue;
        }
        matches.push((distance_sq(caster_pos, entity.pos()), entity.id()));
    }
    matches.sort_unstable();
    matches
        .into_iter()
        .take(max_targets)
        .map(|(_, id)| id)
        .collect()
}

pub fn distance_to(sim: &StableSim<'_>, caster_id: usize, target_id: usize) -> u64 {
    let Some(caster) = sim.get_entity(caster_id) else {
        return u64::MAX;
    };
    let Some(target) = sim.get_entity(target_id) else {
        return u64::MAX;
    };
    distance_sq(caster.pos(), target.pos())
}

pub fn is_target_at_or_below_hp_percent(
    sim: &StableSim<'_>,
    target_id: usize,
    threshold_percent: usize,
) -> bool {
    let Some(target) = sim.get_entity(target_id) else {
        return false;
    };
    if !target.is_alive() || target.is_tower() {
        return false;
    }
    let (current, max) = target.hp();
    current.saturating_mul(100) <= max.saturating_mul(threshold_percent)
}

pub fn is_crowd_controlled(sim: &StableSim<'_>, target_id: usize) -> bool {
    sim.get_entity(target_id)
        .map(|entity| entity.cc_count() > 0)
        .unwrap_or(false)
}

pub fn is_isolated(sim: &StableSim<'_>, target_id: usize, range: u64) -> bool {
    let Some(target) = sim.get_entity(target_id) else {
        return false;
    };
    if !target.is_alive() {
        return false;
    }
    let team = target.team();
    let center = target.pos();
    let range_sq = range.saturating_mul(range);
    for index in 0..sim.entity_count() {
        let Some(ally) = sim.entity_at(index) else {
            continue;
        };
        if ally.id() == target_id || ally.team() != team || !ally.is_alive() {
            continue;
        }
        if distance_sq(ally.pos(), center) <= range_sq {
            return false;
        }
    }
    true
}

pub fn has_shield(sim: &StableSim<'_>, target_id: usize) -> bool {
    sim.get_entity(target_id)
        .map(|entity| entity.is_champion() && entity.shield() > 0)
        .unwrap_or(false)
}

pub fn input_position(sim: &StableSim<'_>, input: InputTargetV1) -> Option<EntityPos> {
    match InputTargetKindV1::from_code(input.kind) {
        Some(InputTargetKindV1::Target) => {
            sim.get_entity(input.target_id).map(|entity| entity.pos())
        }
        Some(InputTargetKindV1::Pos) => Some((input.x, input.y)),
        _ => None,
    }
}

/// Resolves the direction a projectile should use for the current cast.
/// Target and position inputs become caster-to-input vectors; direction input
/// already contains the desired vector.
pub fn input_direction(
    sim: &StableSim<'_>,
    caster_id: usize,
    target_id: usize,
    input: InputTargetV1,
) -> Option<(f64, f64)> {
    let caster_pos = sim.get_entity(caster_id).map(|entity| entity.pos())?;
    let (dx, dy) = match InputTargetKindV1::from_code(input.kind) {
        Some(InputTargetKindV1::Target) => sim
            .get_entity(input.target_id)
            .or_else(|| sim.get_entity(target_id))
            .map(|entity| {
                (
                    entity.pos().0 as f64 - caster_pos.0 as f64,
                    entity.pos().1 as f64 - caster_pos.1 as f64,
                )
            })?,
        Some(InputTargetKindV1::Pos) => (
            input.x as f64 - caster_pos.0 as f64,
            input.y as f64 - caster_pos.1 as f64,
        ),
        Some(InputTargetKindV1::Dir) => (input.dir_x as f64, input.dir_y as f64),
        _ => return None,
    };
    let length = (dx * dx + dy * dy).sqrt();
    (length > f64::EPSILON).then_some((dx / length, dy / length))
}

/// Returns an endpoint at `distance` along the cast direction rotated by the
/// requested angle. Zero degrees preserves the original cast direction.
pub fn rotated_input_endpoint(
    sim: &StableSim<'_>,
    caster_id: usize,
    target_id: usize,
    input: InputTargetV1,
    distance: u64,
    angle_degrees: i32,
) -> Option<EntityPos> {
    let caster_pos = sim.get_entity(caster_id).map(|entity| entity.pos())?;
    let (unit_x, unit_y) = input_direction(sim, caster_id, target_id, input)?;
    let (rotated_x, rotated_y) = rotate_direction(unit_x, unit_y, angle_degrees);
    Some((
        (caster_pos.0 as f64 + rotated_x * distance as f64).max(0.0) as u64,
        (caster_pos.1 as f64 + rotated_y * distance as f64).max(0.0) as u64,
    ))
}

pub fn rotate_direction(unit_x: f64, unit_y: f64, angle_degrees: i32) -> (f64, f64) {
    let angle = (angle_degrees as f64).to_radians();
    (
        unit_x * angle.cos() - unit_y * angle.sin(),
        unit_x * angle.sin() + unit_y * angle.cos(),
    )
}

/// Declarative description of a `SpawnProjectile` native effect node, mirroring
/// the compact shape a Data Graph projectile node already has as JSON. Collects
/// every field the per-champion generated code would otherwise set on
/// [`ProjectileSpawnV1`] by hand, so each call site in `native_effects/*.rs`
/// reads as one struct literal instead of a dozen imperative `spec.field = ...`
/// lines.
pub struct ProjectileEffectSpec<'a> {
    /// Registration name; also the key looked up against Projectile Visuals.
    pub name: &'a str,
    /// Optional companion view effect started (and tracked along the
    /// projectile's live position) when this projectile spawns.
    pub view_effect_name: &'a str,
    pub on_caster: bool,
    pub move_kind: ProjectileMoveKindV1,
    /// Non-zero only for `Linear` movement: rotates the cast direction by this
    /// many degrees before computing the projectile's endpoint.
    pub angle_degrees: i32,
    pub radius: u64,
    pub speed: u64,
    pub penetrate: bool,
    pub attack_type: AttackTypeV1,
    pub casting_type: CastingTypeV1,
    pub casting_target: CastingTargetV1,
}

/// Spawns a projectile from a [`ProjectileEffectSpec`], resolving its launch
/// position/target from the caster, target and cast input the way every
/// generated `SpawnProjectile` node needs to.
pub fn spawn_projectile_effect(
    sim: &mut StableSim<'_>,
    input: InputTargetV1,
    caster_id: usize,
    target_id: usize,
    spec: ProjectileEffectSpec<'_>,
) {
    let mut projectile = ProjectileSpawnV1::default();
    projectile.caster_id = caster_id;
    projectile.team = sim.get_entity(caster_id).map(|entity| entity.team()).unwrap_or(0);
    let caster_pos = sim.get_entity(caster_id).map(|entity| entity.pos()).unwrap_or((0, 0));
    let mut target_pos = sim
        .get_entity(target_id)
        .map(|entity| entity.pos())
        .unwrap_or(caster_pos);
    if spec.angle_degrees != 0 && spec.move_kind == ProjectileMoveKindV1::Linear {
        let direction_pos = input_position(sim, input)
            .or_else(|| sim.get_entity(target_id).map(|entity| entity.pos()))
            .unwrap_or(caster_pos);
        let dx = direction_pos.0.abs_diff(caster_pos.0);
        let dy = direction_pos.1.abs_diff(caster_pos.1);
        let distance = integer_sqrt(dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)));
        if let Some(rotated) =
            rotated_input_endpoint(sim, caster_id, target_id, input, distance, spec.angle_degrees)
        {
            target_pos = rotated;
        }
    }
    projectile.x = if spec.on_caster { caster_pos.0 } else { target_pos.0 };
    projectile.y = if spec.on_caster { caster_pos.1 } else { target_pos.1 };
    projectile.target_id = target_id;
    projectile.target_x = target_pos.0;
    projectile.target_y = target_pos.1;
    projectile.radius = spec.radius;
    projectile.speed = spec.speed;
    projectile.move_kind = spec.move_kind.code();
    projectile.penetrate = spec.penetrate;
    projectile.attack_type = spec.attack_type.code();
    projectile.casting_type = spec.casting_type.code();
    projectile.casting_target = spec.casting_target.code();

    let projectile_name = spec.name.to_string();
    let _ = sim.spawn_projectile(&projectile_name, &projectile_name, &projectile);
    if !spec.view_effect_name.trim().is_empty() {
        crate::state::start_projectile_view_effect(
            sim,
            caster_id,
            &projectile_name,
            projectile.x,
            projectile.y,
            spec.view_effect_name,
            projectile.radius,
            sim.tick(),
        );
    }
}

/// Where a MoveTo effect should aim, mirroring the ability's cast input kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveTargetKind {
    Entity,
    Location,
    Direction,
}

/// Computes the landing point for a MoveTo effect.
///
/// The direction and destination come from the cast input: `Entity` moves
/// toward the resolved target entity, `Location` toward the cast position,
/// and `Direction` along the cast direction. `move_away` flips the direction
/// to move away from the target instead. The total travel is clamped by
/// `min_distance` (floor, can overshoot) and `max_distance` (ceiling), then
/// `offset_x`/`offset_y` are added to the final landing point.
pub fn move_destination(
    mode: MoveTargetKind,
    caster_pos: EntityPos,
    input: InputTargetV1,
    target_id: usize,
    sim: &StableSim<'_>,
    min_distance: u64,
    max_distance: u64,
    offset_x: i64,
    offset_y: i64,
    move_away: bool,
) -> EntityPos {
    let (dx, dy) = match mode {
        MoveTargetKind::Entity => {
            let target_pos = sim
                .get_entity(target_id)
                .map(|entity| entity.pos())
                .unwrap_or(caster_pos);
            (
                target_pos.0 as f64 - caster_pos.0 as f64,
                target_pos.1 as f64 - caster_pos.1 as f64,
            )
        }
        MoveTargetKind::Location => (
            input.x as f64 - caster_pos.0 as f64,
            input.y as f64 - caster_pos.1 as f64,
        ),
        MoveTargetKind::Direction => (input.dir_x as f64, input.dir_y as f64),
    };
    let (dx, dy) = if move_away { (-dx, -dy) } else { (dx, dy) };
    let dir_len = (dx * dx + dy * dy).sqrt();
    let (unit_x, unit_y) = if dir_len > 0.0 {
        (dx / dir_len, dy / dir_len)
    } else {
        (0.0, 0.0)
    };
    let full_dist = match mode {
        MoveTargetKind::Entity | MoveTargetKind::Location => dir_len,
        // A pure direction has no natural endpoint; min/max define the travel.
        MoveTargetKind::Direction => max_distance.max(min_distance) as f64,
    };
    let mut travel = full_dist;
    if max_distance > 0 {
        travel = travel.min(max_distance as f64);
    }
    if min_distance > 0 {
        travel = travel.max(min_distance as f64);
    }
    let x = caster_pos.0 as f64 + unit_x * travel + offset_x as f64;
    let y = caster_pos.1 as f64 + unit_y * travel + offset_y as f64;
    (x.max(0.0) as u64, y.max(0.0) as u64)
}

pub fn chance_percent(
    seed: u64,
    caster_id: usize,
    target_id: usize,
    tick: usize,
    percent: usize,
) -> bool {
    if percent >= 100 {
        return true;
    }
    splitmix64(
        seed ^ ((caster_id as u64) << 40) ^ ((target_id as u64) << 16) ^ tick as u64 ^ 0xa11ce,
    ) % 100
        < percent as u64
}

/// Deterministically selects an index from `0..len` from a host-provided seed.
///
/// The same `(seed, salt, len)` always returns the same index. `salt` lets
/// independent random operations derive different deterministic choices from
/// the same callback seed without introducing nondeterministic state.
pub fn seeded_index(seed: u64, salt: u64, len: usize) -> usize {
    if len <= 1 {
        return 0;
    }
    (splitmix64(seed ^ salt) as usize) % len
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = value;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::{entity_name_matches_value, rotate_direction};

    #[test]
    fn entity_name_matches_numeric_spawn_suffixes() {
        assert!(entity_name_matches_value("clone", "clone"));
        assert!(entity_name_matches_value("clone1", "clone"));
        assert!(entity_name_matches_value("clone12", "clone"));
        assert!(!entity_name_matches_value("clone_extra", "clone"));
        assert!(!entity_name_matches_value("clone1x", "clone"));
    }

    #[test]
    fn zero_degree_rotation_preserves_cast_direction() {
        let (x, y) = rotate_direction(1.0, 0.0, 0);
        assert!((x - 1.0).abs() < 0.000_001);
        assert!(y.abs() < 0.000_001);
    }

    #[test]
    fn twelve_radial_steps_make_one_full_circle() {
        let first = rotate_direction(1.0, 0.0, 0);
        let last = rotate_direction(1.0, 0.0, 330);
        let next = rotate_direction(1.0, 0.0, 360);
        assert!((first.0 - next.0).abs() < 0.000_001);
        assert!((first.1 - next.1).abs() < 0.000_001);
        assert!((first.0 - last.0).abs() > 0.1);
    }
}
