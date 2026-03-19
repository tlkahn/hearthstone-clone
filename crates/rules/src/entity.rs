use std::collections::HashSet;

use crate::card::{CardId, CardTypeData, Keyword};
use crate::types::EntityId;

#[derive(Debug, Clone)]
pub struct MinionEntity {
    pub attack: u32,
    pub health: i32,
    pub max_health: i32,
    pub keywords: HashSet<Keyword>,
    pub summoning_sickness: bool,
    pub attacks_this_turn: u32,
}

#[derive(Debug, Clone)]
pub struct WeaponEntity {
    pub attack: u32,
    pub durability: i32,
}

#[derive(Debug, Clone)]
pub enum EntityData {
    Minion(MinionEntity),
    Spell,
    Weapon(WeaponEntity),
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub id: EntityId,
    pub card_id: CardId,
    pub owner: usize,
    pub data: EntityData,
}

impl Entity {
    pub fn from_card_def(id: EntityId, card_def: &crate::card::CardDef, owner: usize) -> Self {
        let data = match &card_def.card_type {
            CardTypeData::Minion(stats) => EntityData::Minion(MinionEntity {
                attack: stats.attack,
                health: stats.health as i32,
                max_health: stats.health as i32,
                keywords: card_def.keywords.iter().cloned().collect(),
                summoning_sickness: true,
                attacks_this_turn: 0,
            }),
            CardTypeData::Spell => EntityData::Spell,
            CardTypeData::Weapon(stats) => EntityData::Weapon(WeaponEntity {
                attack: stats.attack,
                durability: stats.durability as i32,
            }),
        };

        Entity {
            id,
            card_id: card_def.id.clone(),
            owner,
            data,
        }
    }

    pub fn is_minion(&self) -> bool {
        matches!(self.data, EntityData::Minion(_))
    }

    pub fn as_minion(&self) -> Option<&MinionEntity> {
        match &self.data {
            EntityData::Minion(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_minion_mut(&mut self) -> Option<&mut MinionEntity> {
        match &mut self.data {
            EntityData::Minion(m) => Some(m),
            _ => None,
        }
    }
}
