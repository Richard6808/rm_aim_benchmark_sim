use std::f32::consts::{FRAC_PI_2, PI};

use avian3d::prelude::*;
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

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
    commands.insert_resource(GlobalAmbientLight {
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

    let target_materials = TargetMaterials {
        armor_backing: materials.add(StandardMaterial {
            base_color: Color::srgb(0.018, 0.021, 0.024),
            metallic: 0.2,
            perceptual_roughness: 0.52,
            ..default()
        }),
        light_housing: materials.add(StandardMaterial {
            base_color: Color::srgb(0.035, 0.038, 0.042),
            metallic: 0.55,
            perceptual_roughness: 0.3,
            ..default()
        }),
        lightbar: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.018, 0.012),
            emissive: LinearRgba::new(14.0, 0.01, 0.005, 1.0),
            perceptual_roughness: 0.2,
            ..default()
        }),
        center: materials.add(StandardMaterial {
            base_color: Color::srgb(0.10, 0.115, 0.125),
            metallic: 0.08,
            perceptual_roughness: 0.72,
            ..default()
        }),
        digit: materials.add(StandardMaterial {
            base_color: Color::srgb(0.92, 0.95, 0.98),
            emissive: LinearRgba::new(1.8, 1.9, 2.0, 1.0),
            perceptual_roughness: 0.4,
            ..default()
        }),
        chassis: materials.add(StandardMaterial {
            base_color: Color::srgb(0.055, 0.065, 0.072),
            metallic: 0.62,
            perceptual_roughness: 0.32,
            ..default()
        }),
        aluminum: materials.add(StandardMaterial {
            base_color: Color::srgb(0.30, 0.33, 0.35),
            metallic: 0.82,
            perceptual_roughness: 0.24,
            ..default()
        }),
        rubber: materials.add(StandardMaterial {
            base_color: Color::srgb(0.018, 0.020, 0.022),
            perceptual_roughness: 0.92,
            ..default()
        }),
    };

    spawn_target_body(commands, meshes, target, &target_materials);

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
            &target_materials,
        );
        commands.entity(target).add_child(plate);
    }

    commands.insert_resource(TargetRuntime {
        origin,
        phase_start_s: 0.0,
        angular_speed_rad_s: config.target.angular_speed_rad_s,
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

#[derive(Clone)]
struct TargetMaterials {
    armor_backing: Handle<StandardMaterial>,
    light_housing: Handle<StandardMaterial>,
    lightbar: Handle<StandardMaterial>,
    center: Handle<StandardMaterial>,
    digit: Handle<StandardMaterial>,
    chassis: Handle<StandardMaterial>,
    aluminum: Handle<StandardMaterial>,
    rubber: Handle<StandardMaterial>,
}

fn spawn_target_body(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    target: Entity,
    materials: &TargetMaterials,
) {
    let body_parts = [
        (
            Cuboid::new(0.48, 0.075, 0.43),
            materials.chassis.clone(),
            Transform::from_xyz(0.0, 0.105, 0.0),
        ),
        (
            Cuboid::new(0.42, 0.115, 0.38),
            materials.aluminum.clone(),
            Transform::from_xyz(0.0, 0.19, 0.0),
        ),
        (
            Cuboid::new(0.33, 0.055, 0.33),
            materials.chassis.clone(),
            Transform::from_xyz(0.0, 0.275, 0.0)
                .with_rotation(Quat::from_rotation_y(PI * 0.25)),
        ),
        (
            Cuboid::new(0.205, 0.20, 0.205),
            materials.chassis.clone(),
            Transform::from_xyz(0.0, 0.405, 0.0),
        ),
    ];

    for (shape, material, transform) in body_parts {
        spawn_target_visual(commands, meshes, target, shape, material, transform);
    }

    let wheel_mesh = meshes.add(Cylinder::new(0.082, 0.072));
    let hub_mesh = meshes.add(Cylinder::new(0.038, 0.075));
    for x in [-0.265, 0.265] {
        for z in [-0.155, 0.155] {
            let wheel = commands
                .spawn((
                    Mesh3d(wheel_mesh.clone()),
                    MeshMaterial3d(materials.rubber.clone()),
                    Transform::from_xyz(x, 0.092, z)
                        .with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
                ))
                .id();
            commands.entity(target).add_child(wheel);

            let hub = commands
                .spawn((
                    Mesh3d(hub_mesh.clone()),
                    MeshMaterial3d(materials.aluminum.clone()),
                    Transform::from_xyz(x.signum() * 0.004, 0.0, 0.0),
                ))
                .id();
            commands.entity(wheel).add_child(hub);
        }
    }

    let yaw_ring = commands
        .spawn((
            Mesh3d(meshes.add(Cylinder::new(0.15, 0.045))),
            MeshMaterial3d(materials.aluminum.clone()),
            Transform::from_xyz(0.0, 0.545, 0.0),
        ))
        .id();
    commands.entity(target).add_child(yaw_ring);

    spawn_target_visual(
        commands,
        meshes,
        target,
        Cuboid::new(0.20, 0.105, 0.16),
        materials.chassis.clone(),
        Transform::from_xyz(0.0, 0.615, 0.0),
    );
    spawn_target_visual(
        commands,
        meshes,
        target,
        Cylinder::new(0.018, 0.30),
        materials.aluminum.clone(),
        Transform::from_xyz(0.0, 0.64, 0.20)
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    );
    spawn_target_visual(
        commands,
        meshes,
        target,
        Cylinder::new(0.027, 0.055),
        materials.chassis.clone(),
        Transform::from_xyz(0.0, 0.64, 0.365)
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    );

    let post_mesh = meshes.add(Cuboid::new(0.022, 0.21, 0.022));
    for x in [-0.15, 0.15] {
        for z in [-0.15, 0.15] {
            let post = commands
                .spawn((
                    Mesh3d(post_mesh.clone()),
                    MeshMaterial3d(materials.aluminum.clone()),
                    Transform::from_xyz(x, 0.39, z),
                ))
                .id();
            commands.entity(target).add_child(post);
        }
    }
}

fn spawn_target_visual(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    parent: Entity,
    shape: impl Into<Mesh>,
    material: Handle<StandardMaterial>,
    transform: Transform,
) {
    let entity = commands
        .spawn((
            Mesh3d(meshes.add(shape)),
            MeshMaterial3d(material),
            transform,
        ))
        .id();
    commands.entity(parent).add_child(entity);
}

fn spawn_armor_plate(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    config: &SimConfig,
    index: u8,
    translation: Vec3,
    yaw: f32,
    materials: &TargetMaterials,
) -> Entity {
    const ARMOR_UPWARD_TILT_DEG: f32 = 15.0;

    let module_w = config.target.armor_width_m;
    let face_w = config.target.armor_face_width_m.min(module_w);
    let h = config.target.armor_height_m;
    let t = config.target.armor_thickness_m;
    let light_w = config.target.armor_light_width_m.min(module_w * 0.25);
    let light_h = config.target.armor_light_height_m.min(h * 0.75);
    let emissive_w = config.target.armor_light_emissive_width_m.min(light_w);
    let face_depth = (t * 0.25).max(0.003);
    let front_z = face_depth * 0.5;
    // Local +Z is the armor's outward normal. Negative local-X rotation raises that normal,
    // so every plate faces outward and upward after its yaw is applied.
    let rotation = Quat::from_rotation_y(yaw)
        * Quat::from_rotation_x(-ARMOR_UPWARD_TILT_DEG.to_radians());

    let plate = commands
        .spawn((
            ArmorPlate { index },
            Mesh3d(meshes.add(chamfered_box_mesh(face_w, h, face_depth, 0.007))),
            MeshMaterial3d(materials.armor_backing.clone()),
            Collider::cuboid(module_w, h, t),
            CollisionEventsEnabled,
            Transform::from_translation(translation).with_rotation(rotation),
        ))
        .observe(armor_hit_observer)
        .id();

    let bracket = commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(0.095, 0.038, t * 2.4))),
            MeshMaterial3d(materials.aluminum.clone()),
            Transform::from_xyz(0.0, -0.012, -t * 1.15),
        ))
        .id();
    commands.entity(plate).add_child(bracket);

    let center_w = 0.082_f32.min(face_w * 0.75);
    let center_h = 0.068_f32.min(h * 0.65);
    let center_plate = commands
        .spawn((
            Mesh3d(meshes.add(chamfered_box_mesh(
                center_w,
                center_h,
                face_depth * 0.42,
                0.004,
            ))),
            MeshMaterial3d(materials.center.clone()),
            Transform::from_xyz(0.0, 0.0, front_z + face_depth * 0.23),
        ))
        .id();
    commands.entity(plate).add_child(center_plate);

    let housing_w = light_w + 0.002;
    let housing_h = light_h + 0.004;
    let housing_t = (t * 0.55).max(0.004);
    let light_x = face_w * 0.5 - light_w * 0.5 + 0.0014;
    for side in [-1.0_f32, 1.0] {
        let housing = commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(housing_w, housing_h, housing_t))),
                MeshMaterial3d(materials.light_housing.clone()),
                Transform::from_xyz(side * light_x, 0.0, front_z + housing_t * 0.36),
            ))
            .id();
        commands.entity(plate).add_child(housing);

        let strip = commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(emissive_w, light_h, housing_t * 0.42))),
                MeshMaterial3d(materials.lightbar.clone()),
                Transform::from_xyz(0.0, 0.0, housing_t * 0.62),
            ))
            .id();
        commands.entity(housing).add_child(strip);
    }

    spawn_armor_digit(
        commands, meshes, plate, center_w, center_h, face_depth, materials,
    );

    let screw_mesh = meshes.add(Cylinder::new(0.003, 0.002));
    for x in [-0.0475, 0.0475] {
        for y in [-0.0475, 0.0475] {
            let screw = commands
                .spawn((
                    Mesh3d(screw_mesh.clone()),
                    MeshMaterial3d(materials.aluminum.clone()),
                    Transform::from_xyz(x, y, front_z + 0.001)
                        .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
                ))
                .id();
            commands.entity(plate).add_child(screw);
        }
    }

    let tab_mesh = meshes.add(Cuboid::new(0.0105, 0.004, 0.007));
    for x in [-0.045, 0.045] {
        let tab = commands
            .spawn((
                Mesh3d(tab_mesh.clone()),
                MeshMaterial3d(materials.armor_backing.clone()),
                Transform::from_xyz(x, h * 0.5 + 0.0015, -0.001),
            ))
            .id();
        commands.entity(plate).add_child(tab);
    }

    plate
}

fn spawn_armor_digit(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    plate: Entity,
    center_w: f32,
    center_h: f32,
    t: f32,
    materials: &TargetMaterials,
) {
    let digit_w = center_w * 0.30;
    let digit_h = center_h * 0.70;
    let stroke = center_h * 0.075;
    let z = t * 1.04;
    let horizontal_mesh = meshes.add(Cuboid::new(digit_w, stroke, t * 0.18));
    let vertical_mesh = meshes.add(Cuboid::new(stroke, digit_h * 0.46, t * 0.18));

    for y in [-digit_h * 0.5, 0.0, digit_h * 0.5] {
        let segment = commands
            .spawn((
                Mesh3d(horizontal_mesh.clone()),
                MeshMaterial3d(materials.digit.clone()),
                Transform::from_xyz(0.0, y, z),
            ))
            .id();
        commands.entity(plate).add_child(segment);
    }
    for y in [-digit_h * 0.25, digit_h * 0.25] {
        let segment = commands
            .spawn((
                Mesh3d(vertical_mesh.clone()),
                MeshMaterial3d(materials.digit.clone()),
                Transform::from_xyz(digit_w * 0.5, y, z),
            ))
            .id();
        commands.entity(plate).add_child(segment);
    }
}

fn chamfered_box_mesh(width: f32, height: f32, depth: f32, chamfer: f32) -> Mesh {
    let half_w = width * 0.5;
    let half_h = height * 0.5;
    let half_d = depth * 0.5;
    let c = chamfer.min(half_w * 0.45).min(half_h * 0.45);
    let outline = [
        Vec2::new(-half_w + c, -half_h),
        Vec2::new(half_w - c, -half_h),
        Vec2::new(half_w, -half_h + c),
        Vec2::new(half_w, half_h - c),
        Vec2::new(half_w - c, half_h),
        Vec2::new(-half_w + c, half_h),
        Vec2::new(-half_w, half_h - c),
        Vec2::new(-half_w, -half_h + c),
    ];

    let mut positions = Vec::with_capacity(48);
    let mut normals = Vec::with_capacity(48);
    let mut uvs = Vec::with_capacity(48);
    let mut indices = Vec::with_capacity(84);

    for point in outline {
        positions.push([point.x, point.y, half_d]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push([point.x / width + 0.5, 0.5 - point.y / height]);
    }
    for point in outline {
        positions.push([point.x, point.y, -half_d]);
        normals.push([0.0, 0.0, -1.0]);
        uvs.push([0.5 - point.x / width, 0.5 - point.y / height]);
    }
    for i in 1..7 {
        indices.extend_from_slice(&[0, i, i + 1]);
        indices.extend_from_slice(&[8, 8 + i + 1, 8 + i]);
    }

    for i in 0..8 {
        let next = (i + 1) % 8;
        let edge = outline[next] - outline[i];
        let normal = Vec2::new(edge.y, -edge.x).normalize();
        let base = positions.len() as u32;
        positions.extend_from_slice(&[
            [outline[i].x, outline[i].y, half_d],
            [outline[i].x, outline[i].y, -half_d],
            [outline[next].x, outline[next].y, -half_d],
            [outline[next].x, outline[next].y, half_d],
        ]);
        normals.extend_from_slice(&[[normal.x, normal.y, 0.0]; 4]);
        uvs.extend_from_slice(&[[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]);
        indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
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
