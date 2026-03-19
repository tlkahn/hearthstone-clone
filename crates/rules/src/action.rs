use crate::types::{EntityId, PlayerId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    EndTurn {
        player: PlayerId,
    },
    PlayCard {
        player: PlayerId,
        hand_index: usize,
        position: usize,
        target: Option<EntityId>,
    },
    Attack {
        player: PlayerId,
        attacker: EntityId,
        defender: EntityId,
    },
}

impl Action {
    pub fn player(&self) -> PlayerId {
        match self {
            Action::EndTurn { player } => *player,
            Action::PlayCard { player, .. } => *player,
            Action::Attack { player, .. } => *player,
        }
    }
}
