//! Command-line argument parser for launching test rooms, model viewers, and FMV players (`CliArgs`).

use sh2_ui::menu::TestRoomKind;

#[derive(Debug, Default, Clone)]
pub(crate) struct CliArgs {
    pub initial_test_room: Option<TestRoomKind>,
    pub initial_fog_debug: bool,
    pub initial_viewer_model: Option<String>,
    pub initial_fmv_movie: Option<String>,
}

impl CliArgs {
    pub(crate) fn parse_from_env() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let mut initial_test_room = None;
        let mut initial_fog_debug = false;
        let mut initial_viewer_model = None;
        let mut initial_fmv_movie = None;

        let mut i = 1;
        while i < args.len() {
            let arg = &args[i];
            if arg == "--test-fog" {
                initial_fog_debug = true;
                if initial_test_room.is_none() {
                    initial_test_room = Some(TestRoomKind::SyntheticFogRunway);
                }
            } else if arg == "--model-viewer" {
                initial_test_room = Some(TestRoomKind::ModelAndTextureViewer);
                if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    initial_viewer_model = Some(args[i + 1].clone());
                    i += 1;
                }
            } else if let Some(model_str) = arg.strip_prefix("--model-viewer=") {
                initial_test_room = Some(TestRoomKind::ModelAndTextureViewer);
                if !model_str.is_empty() {
                    initial_viewer_model = Some(model_str.to_string());
                }
            } else if arg == "--fmv-viewer" || arg == "--fmv" {
                initial_test_room = Some(TestRoomKind::FmvCutsceneViewer);
                if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    initial_fmv_movie = Some(args[i + 1].clone());
                    i += 1;
                }
            } else if let Some(movie_str) = arg.strip_prefix("--fmv-viewer=") {
                initial_test_room = Some(TestRoomKind::FmvCutsceneViewer);
                if !movie_str.is_empty() {
                    initial_fmv_movie = Some(movie_str.to_string());
                }
            } else if arg == "--grapple" || arg == "--grapples" || arg == "--grapple-viewer" {
                initial_test_room = Some(TestRoomKind::GrappleAnimationRoom);
            } else if arg == "--audio" || arg == "--audio-debug" || arg == "--bgm" {
                initial_test_room = Some(TestRoomKind::AudioDebugRoom);
            } else if arg == "--sfx" || arg == "--surface-sfx" || arg == "--footsteps" {
                initial_test_room = Some(TestRoomKind::SurfaceSfxTestRoom);
            } else if arg == "--inventory" || arg == "--items" {
                initial_test_room = Some(TestRoomKind::InventoryAndItemsRoom);
            } else if arg == "--puzzles" || arg == "--cutaway" || arg == "--events" {
                initial_test_room = Some(TestRoomKind::PuzzleAndCutawayRoom);
            } else if let Some(room_str) = arg
                .strip_prefix("--test-room=")
                .or_else(|| arg.strip_prefix("--room="))
            {
                initial_test_room = Some(Self::parse_room_alias(room_str, true));
            } else if arg == "--room" && i + 1 < args.len() && !args[i + 1].starts_with("--") {
                initial_test_room = Some(Self::parse_room_alias(&args[i + 1], false));
                i += 1;
            }
            i += 1;
        }

        if let Some(r) = initial_test_room {
            tracing::info!("CLI parameter selected test room: {:?}", r);
        }
        if let Some(ref m) = initial_viewer_model {
            tracing::info!("CLI parameter selected initial model viewer entry: '{}'", m);
        }
        if let Some(ref f) = initial_fmv_movie {
            tracing::info!("CLI parameter selected initial FMV cutscene: '{}'", f);
        }

        Self {
            initial_test_room,
            initial_fog_debug,
            initial_viewer_model,
            initial_fmv_movie,
        }
    }

    fn parse_room_alias(room_str: &str, warn_unknown: bool) -> TestRoomKind {
        match room_str {
            "ob01" | "observation" | "outdoor" => TestRoomKind::ObservationDeck,
            "er01" | "restroom" | "toilet" => TestRoomKind::RestroomStart,
            "rr21" | "boiler" => TestRoomKind::HotelBoilerRoom,
            "runway" | "fog" => TestRoomKind::SyntheticFogRunway,
            "fallback" | "sandbox" => TestRoomKind::FallbackSandbox,
            "viewer" | "model-viewer" | "models" | "bestiary" => {
                TestRoomKind::ModelAndTextureViewer
            }
            "fmv" | "fmv-viewer" | "cutscenes" | "movies" => TestRoomKind::FmvCutsceneViewer,
            "combat" | "arena" | "battle" => TestRoomKind::CombatArena,
            "grapple" | "grapples" | "grapple-room" | "grapple-viewer" => {
                TestRoomKind::GrappleAnimationRoom
            }
            "audio" | "audio-debug" | "bgm" | "sound" => TestRoomKind::AudioDebugRoom,
            "sfx" | "surface-sfx" | "footsteps" | "materials" => TestRoomKind::SurfaceSfxTestRoom,
            "inventory" | "items" | "item-viewer" => TestRoomKind::InventoryAndItemsRoom,
            "puzzles" | "puzzle" | "cutaway" | "events" | "overlays" => {
                TestRoomKind::PuzzleAndCutawayRoom
            }
            _ => {
                if warn_unknown {
                    tracing::warn!(
                        "Unknown test room: '{}', defaulting to Observation Deck",
                        room_str
                    );
                }
                TestRoomKind::ObservationDeck
            }
        }
    }
}
