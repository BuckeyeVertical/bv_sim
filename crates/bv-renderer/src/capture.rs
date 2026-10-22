//! `--screenshot <png>`: save one frame once the world has loaded, then exit.

use std::path::PathBuf;

use bevy::{
    app::AppExit,
    camera::RenderTarget,
    prelude::*,
    render::{
        render_resource::{TextureFormat, TextureUsages},
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
};

use crate::{camera::DebugCamera, worlds::WorldLoad};

/// Wait after loading so render pipelines compile and the finest detail streams in.
const SETTLE_SECS: f32 = 5.0;
/// Give up if the world has not loaded by then (e.g. a missing asset).
const LOAD_TIMEOUT_SECS: f32 = 120.0;
/// Frames can come back all black while pipelines are still compiling (or a
/// macOS window is briefly occluded); retry once a second.
const MAX_ATTEMPTS: u32 = 20;
const RETRY_SECS: f32 = 1.0;

pub struct CapturePlugin {
    pub path: PathBuf,
}

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Capture {
            path: self.path.clone(),
            ready_at: None,
            next_attempt: 0.0,
            attempts: 0,
            pending: false,
            target: None,
        })
        .add_systems(Update, (prepare_target, request_screenshot).chain());
    }
}

#[derive(Resource)]
struct Capture {
    path: PathBuf,
    ready_at: Option<f32>,
    next_attempt: f32,
    attempts: u32,
    pending: bool,
    target: Option<Handle<Image>>,
}

/// Read an offscreen observer image: Metal window surfaces can return black
/// pixels when the window is covered or the application is in the background.
fn prepare_target(
    mut commands: Commands,
    mut capture: ResMut<Capture>,
    camera: Single<Entity, With<DebugCamera>>,
    mut images: ResMut<Assets<Image>>,
) {
    if capture.target.is_some() {
        return;
    }
    let mut image = Image::new_target_texture(1600, 900, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = images.add(image);
    commands
        .entity(*camera)
        .insert(RenderTarget::Image(target.clone().into()));
    capture.target = Some(target);
}

fn request_screenshot(
    mut commands: Commands,
    mut capture: ResMut<Capture>,
    state: Res<State<WorldLoad>>,
    time: Res<Time>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    if *state.get() != WorldLoad::Ready {
        if now > LOAD_TIMEOUT_SECS {
            error!("screenshot: the world did not finish loading in {LOAD_TIMEOUT_SECS} s");
            exit.write(AppExit::error());
        }
        return;
    }
    let ready_at = *capture.ready_at.get_or_insert(now);
    if capture.pending || now - ready_at < SETTLE_SECS || now < capture.next_attempt {
        return;
    }
    if capture.attempts == MAX_ATTEMPTS {
        error!("screenshot: every frame came back black");
        exit.write(AppExit::error());
        return;
    }
    capture.attempts += 1;
    capture.next_attempt = now + RETRY_SECS;
    capture.pending = true;
    commands
        .spawn(Screenshot::image(capture.target.as_ref().unwrap().clone()))
        .observe(save_or_retry);
}

fn save_or_retry(
    captured: On<ScreenshotCaptured>,
    mut capture: ResMut<Capture>,
    mut exit: MessageWriter<AppExit>,
) {
    capture.pending = false;
    let Ok(image) = captured.image.clone().try_into_dynamic() else {
        error!("screenshot: unsupported image format");
        exit.write(AppExit::error());
        return;
    };
    let rgb = image.to_rgb8();
    if rgb.pixels().all(|pixel| pixel.0 == [0, 0, 0]) {
        warn!("screenshot: black frame, retrying");
        return;
    }
    match rgb.save(&capture.path) {
        Ok(()) => {
            info!("screenshot saved to {}", capture.path.display());
            exit.write(AppExit::Success);
        }
        Err(error) => {
            error!(
                "screenshot: cannot save {}: {error}",
                capture.path.display()
            );
            exit.write(AppExit::error());
        }
    }
}
