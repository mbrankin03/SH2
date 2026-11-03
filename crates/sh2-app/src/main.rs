use sh2_app::SilentHill2App;

fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("Initializing Silent Hill 2 Open Port...");
    let app = SilentHill2App::new();
    tracing::info!("Engine initialized. Running: {}", app.is_running);
}
