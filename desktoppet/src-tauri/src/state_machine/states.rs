use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PetState {
    Appear,
    Idle,
    Clicked,
    Think,
    Talk,
    Sleeping,
    Typing,
    Worried,
    Sweating,
    Shutdown,
}

impl PetState {
    /// 仅 loop=false 的状态才能由动画自身退出（瞬态）
    pub fn is_transient(self) -> bool {
        matches!(self, PetState::Appear | PetState::Clicked | PetState::Shutdown)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PetState::Appear => "appear",
            PetState::Idle => "idle",
            PetState::Clicked => "clicked",
            PetState::Think => "think",
            PetState::Talk => "talk",
            PetState::Sleeping => "sleeping",
            PetState::Typing => "typing",
            PetState::Worried => "worried",
            PetState::Sweating => "sweating",
            PetState::Shutdown => "shutdown",
        }
    }
}

impl std::fmt::Display for PetState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
