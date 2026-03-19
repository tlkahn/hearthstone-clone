use serde::{Deserialize, Serialize};

use crate::card::CardId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TargetFilter {
    Any,
    AnyMinion,
    FriendlyMinion,
    EnemyMinion,
    AnyCharacter,
    EnemyCharacter,
    FriendlyCharacter,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TargetSpec {
    None,
    PlayerChoice(TargetFilter),
    Self_,
    All(TargetFilter),
    Random(TargetFilter),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Effect {
    DealDamage {
        amount: u32,
        target: TargetSpec,
    },
    Heal {
        amount: u32,
        target: TargetSpec,
    },
    Summon {
        card_id: CardId,
        count: u32,
        for_opponent: bool,
    },
    DrawCards {
        count: u32,
    },
    BuffMinion {
        attack: i32,
        health: i32,
        target: TargetSpec,
    },
    DestroyMinion {
        target: TargetSpec,
    },
}

impl Effect {
    pub fn requires_target(&self) -> bool {
        match self {
            Effect::DealDamage { target, .. }
            | Effect::Heal { target, .. }
            | Effect::BuffMinion { target, .. }
            | Effect::DestroyMinion { target } => {
                matches!(target, TargetSpec::PlayerChoice(_))
            }
            _ => false,
        }
    }

    pub fn target_spec(&self) -> Option<&TargetSpec> {
        match self {
            Effect::DealDamage { target, .. }
            | Effect::Heal { target, .. }
            | Effect::BuffMinion { target, .. }
            | Effect::DestroyMinion { target } => Some(target),
            _ => None,
        }
    }
}
