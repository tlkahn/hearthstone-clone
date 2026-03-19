use thiserror::Error;

use crate::types::{EntityId, PlayerId};

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum GameError {
    #[error("Not your turn (active player is {active}, got action from {actor})")]
    NotYourTurn { active: PlayerId, actor: PlayerId },

    #[error("Game is already over")]
    GameOver,

    #[error("Not enough mana (have {available}, need {cost})")]
    NotEnoughMana { available: u32, cost: u32 },

    #[error("Board is full (max 7 minions)")]
    BoardFull,

    #[error("Invalid hand index: {index} (hand size: {hand_size})")]
    InvalidHandIndex { index: usize, hand_size: usize },

    #[error("Invalid board position: {position}")]
    InvalidPosition { position: usize },

    #[error("Invalid attacker: {0}")]
    InvalidAttacker(EntityId),

    #[error("Invalid defender: {0}")]
    InvalidDefender(EntityId),

    #[error("Entity has summoning sickness")]
    SummoningSickness,

    #[error("Entity already attacked this turn")]
    AlreadyAttacked,

    #[error("Must attack a taunt minion")]
    MustAttackTaunt,

    #[error("Cannot attack own minion")]
    CannotAttackOwnMinion,

    #[error("Invalid target: {0}")]
    InvalidTarget(EntityId),

    #[error("Target required but not provided")]
    TargetRequired,
}
