use std::f32::consts::*;

use bevy::{
    pbr::{CascadeShadowConfigBuilder, DirectionalLightShadowMap},
    prelude::*,
};

pub fn run() {
    App::new()
        .insert_resource(AmbientLight {
            color: Color::RED,
            brightness: 10. / 5.,
        })
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        .add_plugins(DefaultPlugins)
        .add_startup_system(setup)
        .add_system(animate_light_direction)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(7.5, 7., 5.).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        ..default()
    });

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        // This is a relatively small scene, so use tighter shadow
        // cascade bounds than the default for better quality.
        // We also adjusted the shadow map to be larger since we're
        // only using a single cascade.
        // cascade_shadow_config: CascadeShadowConfigBuilder {
        //     num_cascades: 1,
        //     maximum_distance: 1.6,
        //     ..default()
        // }
        // .into(),
        ..default()
    });

    commands.spawn(SceneBundle {
        scene: asset_server.load("room.glb#Scene0"),
        ..default()
    });
}

fn animate_light_direction(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<DirectionalLight>>,
) {
    for mut transform in &mut query {
        transform.rotation = Quat::from_euler(
            EulerRot::ZYX,
            0.0,
            time.elapsed_seconds() * PI / 5.0,
            -FRAC_PI_4,
        );
    }
}
