use std::collections::HashMap;

use crate::entity::Entity;
use crate::types::*;

#[derive(Debug, Clone)]
pub struct Hero {
    pub hp: i32,
    pub max_hp: i32,
    pub armor: u32,
}

impl Hero {
    pub fn new() -> Self {
        Hero {
            hp: STARTING_HP,
            max_hp: STARTING_HP,
            armor: 0,
        }
    }

    pub fn is_dead(&self) -> bool {
        self.hp <= 0
    }
}

#[derive(Debug, Clone)]
pub struct Player {
    pub hero: Hero,
    pub mana_crystals: u32,
    pub mana: u32,
    pub deck: Vec<EntityId>,
    pub hand: Vec<EntityId>,
    pub board: Vec<EntityId>,
    pub graveyard: Vec<EntityId>,
    pub weapon: Option<EntityId>,
    pub fatigue_counter: u32,
}

impl Player {
    pub fn new() -> Self {
        Player {
            hero: Hero::new(),
            mana_crystals: 0,
            mana: 0,
            deck: Vec::new(),
            hand: Vec::new(),
            board: Vec::new(),
            graveyard: Vec::new(),
            weapon: None,
            fatigue_counter: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GameState {
    pub players: [Player; 2],
    pub entities: HashMap<EntityId, Entity>,
    pub active_player: PlayerId,
    pub turn_number: u32,
    pub game_over: bool,
    pub winner: Option<PlayerId>,
    pub next_entity_id: EntityId,
}

impl GameState {
    pub fn new() -> Self {
        GameState {
            players: [Player::new(), Player::new()],
            entities: HashMap::new(),
            active_player: 0,
            turn_number: 0,
            game_over: false,
            winner: None,
            next_entity_id: 1,
        }
    }

    pub fn alloc_entity_id(&mut self) -> EntityId {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        id
    }

    pub fn opponent(&self, player: PlayerId) -> PlayerId {
        1 - player
    }
}
