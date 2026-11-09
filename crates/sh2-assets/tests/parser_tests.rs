//! Integration test suite for `sh2-assets` binary format parsers, arranged by domain submodule.

pub const ROOT_ASSETS: &str = "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2";

#[path = "parser_suite/audio_dialogue_tests.rs"]
mod audio_dialogue_tests;
#[path = "parser_suite/map_cld_cam_tests.rs"]
mod map_cld_cam_tests;
#[path = "parser_suite/model_anm_tests.rs"]
mod model_anm_tests;
#[path = "parser_suite/texture_video_tests.rs"]
mod texture_video_tests;
