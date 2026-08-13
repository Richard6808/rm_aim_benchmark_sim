use std::time::{SystemTime, UNIX_EPOCH};

use bevy::{
    camera::RenderTarget,
    prelude::*,
    render::{
        render_resource::TextureFormat,
        view::screenshot::{Capturing, Screenshot, ScreenshotCaptured},
    },
};
use image::{ExtendedColorType, codecs::jpeg::JpegEncoder};

use crate::{
    components::{AimCamera, CameraIntrinsics, Gimbal},
    config::SimConfig,
    network::NetworkBridge,
    protocol::CameraFrame,
};

#[derive(Resource, Clone)]
pub struct CameraTarget(pub Handle<Image>);

#[derive(Resource)]
pub struct CameraCaptureTimer(pub Timer);

#[derive(Resource, Default)]
pub struct CameraSequence(pub u64);

pub fn setup_preview_camera(mut commands: Commands) {
    commands.spawn((Camera2d, Camera { order: 0, ..default() }));
}

pub fn setup_capture_target(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    config: Res<SimConfig>,
    gimbal: Single<Entity, With<Gimbal>>,
) {
    let image = Image::new_target_texture(
        config.camera.width,
        config.camera.height,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    let image_handle = images.add(image);

    let fov_y = config.camera.vertical_fov_deg.to_radians();
    let aspect = config.camera.width as f64 / config.camera.height as f64;
    let fov_y_f64 = fov_y as f64;
    let fov_x = 2.0 * ((fov_y_f64 / 2.0).tan() * aspect).atan();
    let intrinsics = CameraIntrinsics {
        width: config.camera.width,
        height: config.camera.height,
        fx: config.camera.width as f64 / (2.0 * (fov_x / 2.0).tan()),
        fy: config.camera.height as f64 / (2.0 * (fov_y_f64 / 2.0).tan()),
        cx: config.camera.width as f64 / 2.0,
        cy: config.camera.height as f64 / 2.0,
    };

    let local = Transform::from_xyz(
        config.camera.right_m,
        config.camera.above_m,
        config.camera.forward_m,
    );

    let camera = commands
        .spawn((
            Camera3d::default(),
            Camera {
                order: -10,
                clear_color: Color::srgb(0.035, 0.04, 0.05).into(),
                ..default()
            },
            RenderTarget::Image(image_handle.clone().into()),
            Projection::Perspective(PerspectiveProjection {
                fov: fov_y,
                near: 0.02,
                far: 100.0,
                ..default()
            }),
            local,
            AimCamera,
        ))
        .id();
    commands.entity(gimbal.into_inner()).add_child(camera);

    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            ..default()
        },
        GlobalZIndex(-100),
        ImageNode::new(image_handle.clone()),
    ));

    commands.insert_resource(CameraTarget(image_handle));
    commands.insert_resource(intrinsics);
    commands.insert_resource(CameraCaptureTimer(Timer::from_seconds(
        1.0 / config.camera.fps.max(1.0),
        TimerMode::Repeating,
    )));
    commands.insert_resource(CameraSequence::default());
}

pub fn request_camera_frames(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<CameraCaptureTimer>,
    target: Res<CameraTarget>,
    capturing: Query<(), With<Capturing>>,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() || !capturing.is_empty() {
        return;
    }

    commands
        .spawn(Screenshot::image(target.0.clone()))
        .observe(on_camera_frame_captured);
}

fn on_camera_frame_captured(
    screenshot: On<ScreenshotCaptured>,
    mut sequence: ResMut<CameraSequence>,
    network: Res<NetworkBridge>,
    config: Res<SimConfig>,
) {
    let Ok(dynamic) = screenshot.image.clone().try_into_dynamic() else {
        return;
    };
    let rgb = dynamic.to_rgb8();
    let mut jpeg = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut jpeg, config.camera.jpeg_quality);
    if encoder
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            ExtendedColorType::Rgb8,
        )
        .is_err()
    {
        return;
    }

    sequence.0 += 1;
    let timestamp_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or_default();
    network.try_send_camera(CameraFrame {
        frame_id: sequence.0,
        timestamp_ns,
        width: rgb.width(),
        height: rgb.height(),
        jpeg,
    });
}
