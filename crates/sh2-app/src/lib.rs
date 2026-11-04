//! sh2-app: Cross-platform entry point and Android NativeActivity presentation layer.

pub mod app;
pub mod menu_controller;

pub use app::SilentHill2App;
pub use menu_controller::AppMenuController;

#[cfg(target_os = "android")]
use android_activity::AndroidApp;

/// Android NativeActivity entry point invoked by the OS.
///
/// Symbol is exported as `android_main` for `android-activity` runtime integration.
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Silent Hill 2 Open Port on Android (AYN Thor)...");
    let engine = SilentHill2App::new();
    tracing::info!(
        "Engine lifecycle initialized. App active: {}",
        engine.is_running
    );

    // Let the app know about the AndroidApp context for windowing and event loops
    let _ = app;
}
