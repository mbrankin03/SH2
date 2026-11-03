//! Game state machines and scenario state.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameScenario {
    #[default]
    LetterFromSilentHeaven,
    BornFromAWish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EngineState {
    #[default]
    Boot,
    Title,
    InGame,
    Paused,
    Cutscene,
}
