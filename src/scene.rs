use std::f32::consts::{FRAC_PI_2, PI};

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    components::{ArmorPlate, Barrel, Gimbal, Muzzle, ShooterRoot, TargetRoot, TargetRuntime},
    config::SimConfig,
    projectile::armor_hit_observer,
};

pub fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    config: Res<SimConfig>,
) {
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 350.0,
        ..default()
    });

    commands.spawn((
        DirectionalLight {
            illuminance: 18_000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -0.9,
            -0.5,
            0.0,
        )),
    ));

    let ground_mesh = meshes.add(Plane3d::default().mesh().size(40.0, 40.0));
    let ground_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.13, 0.14),
        perceptual_roughness: 0.9,
        ..default()
    });
    commands.spawn((
        Mesh3d(ground_mesh),
        MeshMaterial3d(ground_mat),
        RigidBody::Static,
        Collider::cuboid(20.0, 0.02, 20.0),
        Transform::from_xyz(0.0, -0.02, -8.0),
    ));

    spawn_range_markers(&mut commands, &mut meshes, &mut materials);
    spawn_shooter(&mut commands, &mut meshes, &mut materials, &config);
    spawn_target(&mut commands, &mut meshes, &mut materials, &config);
}

fn spawn_shooter(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    config: &SimConfig,
) {
    let metal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.18, 0.20),
        metallic: 0.55,
        perceptual_roughness: 0.35,
        ..default()
    });
    let barrel_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.09, 0.10),
        metallic: 0.7,
        perceptual_roughness: 0.25,
        ..default()
    });

    let shooter = commands
        .spawn((
            ShooterRoot,
            Transform::IDENTITY,
            Visibility::Visible,
        ))
        .id();

    let gimbal = commands
        .spawn((
            Gimbal,
            Mesh3d(meshes.add(Cuboid::new(0.28, 0.16, 0.22))),
            MeshMaterial3d(metal),
            Transform::from_xyz(0.0, config.shooter.gimbal_height_m, 0.0),
        ))
        .id();
    commands.entity(shooter).add_child(gimbal);

    let barrel = commands
        .spawn((
            Barrel,
            Mesh3d(meshes.add(Cylinder::new(
                config.shooter.barrel_radius_m,
                config.shooter.barrel_length_m,
            ))),
            MeshMaterial3d(barrel_mat),
            Transform::from_xyz(0.0, 0.0, -config.shooter.barrel_length_m * 0.5)
                .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
        ))
        .id();
    commands.entity(gimbal).add_child(barrel);

    let muzzle = commands
        .spawn((
            Muzzle,
            Transform::from_xyz(0.0, 0.0, -config.shooter.muzzle_offset_m),
        ))
        .id();
    commands.entity(gimbal).add_child(muzzle);
}

fn spawn_target(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    config: &SimConfig,
) {
    let origin = Vec3::new(0.0, 0.0, -config.target.initial_distance_m);
    let target = commands
        .spawn((
            TargetRoot,
            RigidBody::Kinematic,
            Transform::from_translation(origin),
        ))
        .id();

    let backing = materials.add(StandardMaterial {
        base_color: Color::srgb(0.055, 0.06, 0.065),
        metallic: 0.1,
        perceptual_roughness: 0.65,
        ..default()
    });
    let lightbar = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.035, 0.035),
        emissive: LinearRgba::new(8.0, 0.02, 0.02, 1.0),
        ..default()
    });
    let center = materials.add(StandardMaterial {
        base_color: Color::srgb(0.78, 0.80, 0.82),
        perceptual_roughness: 0.6,
        ..default()
    });

    let r_fb = config.target.front_back_radius_m;
    let r_lr = config.target.left_right_radius_m;
    let y = config.target.armor_center_height_m;

    let armor_poses = [
        (0u8, Vec3::new(0.0, y, r_fb), 0.0),
        (1u8, Vec3::new(r_lr, y, 0.0), FRAC_PI_2),
        (2u8, Vec3::new(0.0, y, -r_fb), PI),
        (3u8, Vec3::new(-r_lr, y, 0.0), -FRAC_PI_2),
    ];

    for (index, translation, yaw) in armor_poses {
        let plate = spawn_armor_plate(
            commands,
            meshes,
            config,
            index,
            translation,
            yaw,
            backing.clone(),
            lightbar.clone(),
            center.clone(),
        );
        commands.entity(target).add_child(plate);
    }

    commands.insert_resource(TargetRuntime {
        origin,
        phase_start_s: 0.0,
        rpm: config.target.rpm,
        translation_speed_mps: config.target.translation_speed_mps,
        half_extent_x_m: config.target.half_extent_x_m,
        half_extent_z_m: config.target.half_extent_z_m,
        path: config.target.path,
        max_hp: config.target.max_hp,
        hp: config.target.max_hp,
        damage_per_hit: config.target.damage_per_hit,
        freeze_when_dead: config.target.freeze_when_dead,
    });
}

#[allow(clippy::too_many_arguments)]
fn spawn_armor_plate(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    config: &SimConfig,
    index: u8,
    translation: Vec3,
    yaw: f32,
    backing: Handle<StandardMaterial>,
    lightbar: Handle<StandardMaterial>,
    center: Handle<StandardMaterial>,
) -> Entity {
    let w = config.target.armor_width_m;
    let h = config.target.armor_height_m;
    let t = config.target.armor_thickness_m;

    let plate = commands
        .spawn((
            ArmorPlate { index },
            Mesh3d(meshes.add(Cuboid::new(w, h, t))),
            MeshMaterial3d(backing),
            Collider::cuboid(w, h, t),
            CollisionEventsEnabled,
            Transform::from_translation(translation).with_rotation(Quat::from_rotation_y(yaw)),
        ))
        .observe(armor_hit_observer)
        .id();

    let bar_w = (w * 0.10).max(0.008);
    let bar_h = h * 0.82;
    let bar_t = t * 1.30;
    for x in [-w * 0.38, w * 0.38] {
        let bar = commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(bar_w, bar_h, bar_t))),
                MeshMaterial3d(lightbar.clone()),
                Transform::from_xyz(x, 0.0, t * 0.58),
            ))
            .id();
        commands.entity(plate).add_child(bar);
    }

    let center_plate = commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(w * 0.48, h * 0.62, t * 1.15))),
            MeshMaterial3d(center),
            Transform::from_xyz(0.0, 0.0, t * 0.56),
        ))
        .id();
    commands.entity(plate).add_child(center_plate);

    plate
}

fn spawn_range_markers(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let marker_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.34, 0.36),
        unlit: true,
        ..default()
    });
    let mesh = meshes.add(Cuboid::new(0.015, 0.006, 0.45));
    for d in 1..=12 {
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(marker_mat.clone()),
            Transform::from_xyz(0.0, 0.004, -(d as f32)),
        ));
    }
}
