//! A Glacier Ridge Metro Park meadow, with trails, ponds and wooded edges.

use std::f32::consts::FRAC_PI_2;

use bevy::{math::Affine2, prelude::*};

use super::DebugCameraStart;
use super::shared::{DaylightPlugin, load_repeating_texture};
use crate::geo::GeoPoint;

pub(super) const HOME: GeoPoint = GeoPoint::new(40.158_615_02, -83.196_968_44);
const MANNEQUIN: Vec2 = Vec2::new(17.6, 22.95);
const SUITCASE: Vec2 = Vec2::new(25.6, 28.95);
const FIELD_SIZE_M: f32 = 1_200.0;

pub(super) struct Glacier2027Plugin;

impl Plugin for Glacier2027Plugin {
    fn build(&self, app: &mut App) {
        let center = ground(Vec2::new(105.0, -20.0), 0.0);
        let camera =
            Transform::from_translation(center + Vec3::Y * 555.0).looking_at(center, Vec3::NEG_X);
        app.insert_resource(DebugCameraStart(camera))
            .add_plugins(DaylightPlugin)
            .add_systems(Startup, (spawn_world, super::ready));
    }
}

// These offsets are meters east and north of HOME. Bevy uses -Z east and -X north.
fn ground(east_north: Vec2, height: f32) -> Vec3 {
    Vec3::new(-east_north.y, height, -east_north.x)
}

fn spawn_world(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let grass = load_repeating_texture(&asset_server, "textures/grass004/color.jpg", true);
    commands.spawn((
        Name::new("Glacier Ridge meadow"),
        Mesh3d(meshes.add(Plane3d::default().mesh().size(FIELD_SIZE_M, FIELD_SIZE_M))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.80, 0.81, 0.64),
            base_color_texture: Some(grass),
            perceptual_roughness: 1.0,
            uv_transform: Affine2::from_scale(Vec2::splat(FIELD_SIZE_M / 65.0)),
            ..default()
        })),
        Transform::from_translation(ground(Vec2::new(80.0, 10.0), 0.0)),
    ));

    spawn_dry_grass(&mut commands, &asset_server, &mut meshes, &mut materials);
    spawn_paths(&mut commands, &mut meshes, &mut materials);
    spawn_ponds(&mut commands, &mut meshes, &mut materials);
    spawn_woodland(&mut commands, &asset_server);
    spawn_targets(&mut commands, &asset_server, &mut meshes, &mut materials);
}

fn spawn_dry_grass(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let patch = meshes.add(Cylinder::new(1.0, 0.004));
    let grass = load_repeating_texture(asset_server, "textures/withered_grass/color.jpg", true);
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.55, 0.57, 0.36, 0.35),
        base_color_texture: Some(grass),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 1.0,
        ..default()
    });
    for (east, north, width, depth, heading) in [
        (-194.0, -75.0, 42.0, 31.0, 0.45),
        (-125.0, 84.0, 24.0, 17.0, -0.34),
        (-66.0, -122.0, 21.0, 16.0, 0.80),
        (-40.0, 67.0, 12.0, 7.0, 1.0),
        (5.0, 95.0, 19.0, 8.0, -0.55),
        (77.0, -65.0, 18.0, 12.0, 0.51),
        (97.0, 47.0, 13.0, 8.0, -0.26),
        (141.0, -144.0, 35.0, 19.0, 0.37),
        (151.0, 94.0, 20.0, 12.0, 0.22),
        (348.0, 91.0, 26.0, 17.0, 0.74),
        (383.0, -146.0, 38.0, 20.0, -0.16),
    ] {
        commands.spawn((
            Name::new("Dry meadow grass"),
            Mesh3d(patch.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(ground(Vec2::new(east, north), 0.003))
                .with_rotation(Quat::from_rotation_y(heading))
                .with_scale(Vec3::new(depth, 1.0, width)),
        ));
    }
}

fn spawn_paths(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let segment = meshes.add(Cuboid::new(1.0, 0.015, 1.0));
    let paving = materials.add(StandardMaterial {
        base_color: Color::srgb(0.44, 0.43, 0.36),
        perceptual_roughness: 1.0,
        ..default()
    });
    let trail = [
        Vec2::new(-490.0, 151.0),
        Vec2::new(-255.0, 132.0),
        Vec2::new(-116.0, 148.0),
        Vec2::new(70.0, 161.0),
        Vec2::new(220.0, 184.0),
        Vec2::new(362.0, 206.0),
        Vec2::new(452.0, 158.0),
        Vec2::new(498.0, 86.0),
        Vec2::new(487.0, -79.0),
        Vec2::new(458.0, -190.0),
        Vec2::new(475.0, -300.0),
    ];
    for pair in trail.windows(2) {
        let start = ground(pair[0], 0.008);
        let end = ground(pair[1], 0.008);
        let delta = end - start;
        commands.spawn((
            Name::new("Meadow trail"),
            Mesh3d(segment.clone()),
            MeshMaterial3d(paving.clone()),
            Transform::from_translation(start.midpoint(end))
                .with_rotation(Quat::from_rotation_y((-delta.z).atan2(delta.x)))
                .with_scale(Vec3::new(delta.length() + 3.0, 1.0, 3.0)),
        ));
    }
}

fn spawn_ponds(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let oval = meshes.add(Cylinder::new(1.0, 0.012));
    let bank = materials.add(StandardMaterial {
        base_color: Color::srgb(0.34, 0.36, 0.22),
        perceptual_roughness: 1.0,
        ..default()
    });
    let water = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.24, 0.19),
        perceptual_roughness: 0.36,
        ..default()
    });
    for (center, east_radius, north_radius, heading) in [
        (Vec2::new(253.0, 143.0), 45.0, 26.0, 0.25),
        (Vec2::new(301.0, -30.0), 38.0, 29.0, -0.3),
    ] {
        for (name, material, height, size) in [
            ("Pond bank", &bank, 0.008, 1.0),
            ("Pond surface", &water, 0.023, 0.75),
        ] {
            commands.spawn((
                Name::new(name),
                Mesh3d(oval.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(ground(center, height))
                    .with_rotation(Quat::from_rotation_y(heading))
                    .with_scale(Vec3::new(north_radius * size, 1.0, east_radius * size)),
            ));
        }
    }
}

fn spawn_woodland(commands: &mut Commands, asset_server: &AssetServer) {
    let tree = asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/tree1.glb"));
    // A wooded southwest edge and the hedgerow south of the ponds leave the meadow open.
    for (origin, columns, rows) in [
        (Vec2::new(-375.0, -300.0), 8, 8),
        (Vec2::new(159.0, -222.0), 19, 2),
        (Vec2::new(528.0, 148.0), 3, 6),
    ] {
        for row in 0..rows {
            for column in 0..columns {
                let index = (row * columns + column) as f32;
                let offset = Vec2::new(
                    column as f32 * 18.0 + (index * 2.3).sin() * 7.0,
                    row as f32 * 20.0 + (index * 1.7).cos() * 7.0,
                );
                let scale = 0.58 + 0.22 * (index * 2.1).sin();
                commands.spawn((
                    Name::new("Glacier Ridge tree"),
                    WorldAssetRoot(tree.clone()),
                    Transform::from_translation(ground(origin + offset, 0.0))
                        .with_rotation(Quat::from_rotation_y(index * 2.4))
                        .with_scale(Vec3::splat(scale)),
                ));
            }
        }
    }
}

fn spawn_targets(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mannequin = asset_server
        .load(GltfAssetLabel::Scene(0).from_asset("models/polo_shirt_mannequin_optimized.glb"));
    commands.spawn((
        Name::new("Glacier target: mannequin"),
        WorldAssetRoot(mannequin),
        Transform::from_translation(ground(MANNEQUIN, 0.19))
            .with_rotation(Quat::from_rotation_y(0.45) * Quat::from_rotation_x(-FRAC_PI_2))
            .with_scale(Vec3::splat(1.75 / 1.928_569)),
    ));

    let shell = materials.add(StandardMaterial {
        base_color: Color::srgb(0.74, 0.07, 0.32),
        perceptual_roughness: 0.65,
        ..default()
    });
    let trim = materials.add(Color::srgb(0.10, 0.09, 0.12));
    let strap = meshes.add(Cuboid::new(0.035, 0.01, 0.44));
    commands
        .spawn((
            Name::new("Glacier target: suitcase"),
            Transform::from_translation(ground(SUITCASE, 0.0))
                .with_rotation(Quat::from_rotation_y(-0.65)),
            Visibility::default(),
        ))
        .with_children(|case| {
            case.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.68, 0.22, 0.44))),
                MeshMaterial3d(shell),
                Transform::from_xyz(0.0, 0.11, 0.0),
            ));
            for x in [-0.19, 0.19] {
                case.spawn((
                    Mesh3d(strap.clone()),
                    MeshMaterial3d(trim.clone()),
                    Transform::from_xyz(x, 0.224, 0.0),
                ));
            }
            case.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.22, 0.045, 0.05))),
                MeshMaterial3d(trim),
                Transform::from_xyz(0.0, 0.13, 0.24),
            ));
        });
}
