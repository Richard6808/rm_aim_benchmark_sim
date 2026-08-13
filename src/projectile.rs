use std::time::Duration;

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    components::{
        AcceptedBenchmarkProjectile, EvaluationGate, Gimbal, GimbalState, Muzzle, Projectile,
        ProjectileLifetime, ScoreBoard, ShotClock, TargetRuntime,
    },
    config::SimConfig,
};

#[derive(Resource, Clone)]
pub struct ProjectileVisual {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

pub fn setup_projectiles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    config: Res<SimConfig>,
) {
    let radius = config.projectile.diameter_m * 0.5;
    let mesh = meshes.add(Sphere::new(radius));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 1.0, 0.12),
        emissive: LinearRgba::new(0.20, 10.0, 0.25, 1.0),
        ..default()
    });
    commands.insert_resource(ProjectileVisual { mesh, material });
    commands.insert_resource(ShotClock {
        next_shot_id: 1,
        cooldown: Timer::new(
            Duration::from_secs_f32(config.projectile.cooldown_s.max(0.001)),
            TimerMode::Once,
        ),
    });
    commands.insert_resource(ScoreBoard::default());
    commands.insert_resource(EvaluationGate::default());
}

#[allow(clippy::too_many_arguments)]
pub fn launch_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<SimConfig>,
    visual: Res<ProjectileVisual>,
    mut clock: ResMut<ShotClock>,
    gimbal_state: Res<GimbalState>,
    gate: Res<EvaluationGate>,
    mut score: ResMut<ScoreBoard>,
    gimbal: Single<&GlobalTransform, With<Gimbal>>,
    muzzle: Single<&GlobalTransform, With<Muzzle>>,
) {
    clock.cooldown.tick(time.delta());
    if !gimbal_state.fire_latched || !clock.cooldown.is_finished() {
        return;
    }
    if gate.benchmark_active {
        if !gate.accepting || score.shots >= gate.rounds_limit as u64 {
            return;
        }
    }

    clock.cooldown.reset();
    let shot_id = clock.next_shot_id;
    clock.next_shot_id += 1;

    let direction = (gimbal.rotation() * Vec3::NEG_Z).normalize_or_zero();
    if direction == Vec3::ZERO {
        return;
    }

    let mut entity = commands.spawn((
        Projectile {
            shot_id,
            counted_hit: false,
        },
        ProjectileLifetime(Timer::from_seconds(
            config.projectile.lifetime_s,
            TimerMode::Once,
        )),
        RigidBody::Dynamic,
        Collider::sphere(config.projectile.diameter_m * 0.5),
        Mass(config.projectile.mass_kg),
        LinearDamping(config.projectile.linear_damping),
        Restitution::new(0.15),
        SweptCcd::default(),
        LinearVelocity(direction * config.projectile.speed_mps),
        Transform::from_translation(muzzle.translation()),
        Mesh3d(visual.mesh.clone()),
        MeshMaterial3d(visual.material.clone()),
    ));
    if gate.benchmark_active {
        entity.insert(AcceptedBenchmarkProjectile {
            trial_id: gate.trial_id,
        });
    }
    score.note_shot(time.elapsed_secs_f64());
}

pub fn despawn_expired_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    mut projectiles: Query<(Entity, &mut ProjectileLifetime)>,
) {
    for (entity, mut lifetime) in &mut projectiles {
        lifetime.0.tick(time.delta());
        if lifetime.0.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn armor_hit_observer(
    collision: On<CollisionStart>,
    mut commands: Commands,
    time: Res<Time>,
    config: Res<SimConfig>,
    gate: Res<EvaluationGate>,
    mut target: ResMut<TargetRuntime>,
    mut score: ResMut<ScoreBoard>,
    mut projectiles: Query<(
        &mut Projectile,
        Option<&AcceptedBenchmarkProjectile>,
    )>,
) {
    let other = collision.collider2;
    let Ok((mut projectile, accepted)) = projectiles.get_mut(other) else {
        return;
    };
    if projectile.counted_hit {
        return;
    }
    if gate.benchmark_active
        && accepted.is_none_or(|accepted| accepted.trial_id != gate.trial_id)
    {
        return;
    }

    projectile.counted_hit = true;
    let now = time.elapsed_secs_f64();
    let damage = target.damage_per_hit.max(0.0);
    if damage <= 0.0 {
        return;
    }

    if score.note_hit(projectile.shot_id, now, damage, config.benchmark.dps_window_s) {
        target.hp = (target.hp - damage).max(0.0);
        if target.hp <= 0.0 && score.kill_time_s.is_none() {
            score.kill_time_s = score.trial_start_s.map(|t0| now - t0);
        }
    }
    commands.entity(other).despawn();
}
