use std::sync::Arc;

use godot::builtin::{Array, GString, VarDictionary};
use godot::prelude::*;
use hs_rules::card::{CardTypeData, Keyword};
use hs_rules::card_loader::CardRegistry;
use hs_rules::engine::GameEngine;
use hs_rules::entity::EntityData;
use hs_rules::event::Event;
use hs_rules::types::{EntityId, PlayerId};
use hs_rules::Action;

#[derive(GodotClass)]
#[class(base=Node)]
pub struct GameBridge {
    engine: Option<GameEngine>,
    registry: Option<Arc<CardRegistry>>,
    base: Base<Node>,
}

#[godot_api]
impl INode for GameBridge {
    fn init(base: Base<Node>) -> Self {
        Self {
            engine: None,
            registry: None,
            base,
        }
    }
}

#[godot_api]
impl GameBridge {
    #[func]
    pub fn start_game(
        &mut self,
        deck_p0: Array<GString>,
        deck_p1: Array<GString>,
    ) -> Array<VarDictionary> {
        // Load registry
        let project_path = godot::classes::ProjectSettings::singleton()
            .globalize_path("res://..")
            .to_string();
        let data_dir = std::path::PathBuf::from(project_path)
            .join("data")
            .join("cards");
        let registry = match CardRegistry::load_from_directory(&data_dir) {
            Ok(r) => {
                godot_print!("GameBridge: loaded {} cards", r.count());
                Arc::new(r)
            }
            Err(errors) => {
                for e in &errors {
                    godot_error!("GameBridge: {}", e);
                }
                return Array::new();
            }
        };

        let d0: Vec<String> = deck_p0.iter_shared().map(|s| s.to_string()).collect();
        let d1: Vec<String> = deck_p1.iter_shared().map(|s| s.to_string()).collect();

        let rng = Box::new(rand::thread_rng());
        let (engine, events) = GameEngine::new_game(registry.clone(), &d0, &d1, rng);

        self.registry = Some(registry);
        self.engine = Some(engine);

        events_to_array(&events)
    }

    #[func]
    pub fn play_card(
        &mut self,
        hand_index: i64,
        position: i64,
        target: i64,
    ) -> VarDictionary {
        let Some(engine) = &mut self.engine else {
            return error_result("No game in progress");
        };

        let player = engine.state().active_player;
        let target_opt = if target < 0 {
            None
        } else {
            Some(target as EntityId)
        };

        let action = Action::PlayCard {
            player,
            hand_index: hand_index as usize,
            position: position as usize,
            target: target_opt,
        };

        match engine.process_action(action) {
            Ok(events) => ok_result(&events),
            Err(e) => error_result(&e.to_string()),
        }
    }

    #[func]
    pub fn attack(&mut self, attacker_id: i64, defender_id: i64) -> VarDictionary {
        let Some(engine) = &mut self.engine else {
            return error_result("No game in progress");
        };

        let player = engine.state().active_player;
        let action = Action::Attack {
            player,
            attacker: i64_to_entity_id(attacker_id),
            defender: i64_to_entity_id(defender_id),
        };

        match engine.process_action(action) {
            Ok(events) => ok_result(&events),
            Err(e) => error_result(&e.to_string()),
        }
    }

    #[func]
    pub fn end_turn(&mut self) -> VarDictionary {
        let Some(engine) = &mut self.engine else {
            return error_result("No game in progress");
        };

        let player = engine.state().active_player;
        let action = Action::EndTurn { player };

        match engine.process_action(action) {
            Ok(events) => ok_result(&events),
            Err(e) => error_result(&e.to_string()),
        }
    }

    #[func]
    pub fn get_hand(&self, player: i64) -> Array<VarDictionary> {
        let Some(engine) = &self.engine else {
            return Array::new();
        };
        let Some(registry) = &self.registry else {
            return Array::new();
        };
        let p = player as PlayerId;
        let state = engine.state();
        let active = state.active_player;
        let is_local = p == active;

        let mut arr = Array::new();
        for (idx, &eid) in state.players[p].hand.iter().enumerate() {
            if is_local {
                // Face-up: full card data
                let mut dict = VarDictionary::new();
                if let Some(entity) = state.entities.get(&eid) {
                    if let Some(card_def) = registry.get(&entity.card_id) {
                        dict.set("card_id", GString::from(card_def.id.as_str()));
                        dict.set("name", GString::from(card_def.name.as_str()));
                        dict.set("mana_cost", card_def.mana_cost as i64);
                        dict.set("text", GString::from(card_def.text.as_str()));
                        dict.set("art", GString::from(card_def.art.as_str()));

                        match &card_def.card_type {
                            CardTypeData::Minion(stats) => {
                                dict.set("card_type", GString::from("minion"));
                                dict.set("attack", stats.attack as i64);
                                dict.set("health", stats.health as i64);
                            }
                            CardTypeData::Spell => {
                                dict.set("card_type", GString::from("spell"));
                            }
                            CardTypeData::Weapon(stats) => {
                                dict.set("card_type", GString::from("weapon"));
                                dict.set("attack", stats.attack as i64);
                                dict.set("durability", stats.durability as i64);
                            }
                        }

                        let mut kw_arr = Array::<GString>::new();
                        for kw in &card_def.keywords {
                            kw_arr.push(&GString::from(keyword_str(kw)));
                        }
                        dict.set("keywords", kw_arr);
                        dict.set("rarity", GString::from(rarity_str(&card_def.rarity)));
                    }
                }
                dict.set("entity_id", entity_id_to_i64(eid));
                dict.set("hand_index", idx as i64);
                dict.set("playable", engine.is_card_playable(p, idx));
                arr.push(&dict);
            } else {
                // Face-down
                let mut dict = VarDictionary::new();
                dict.set("face_down", true);
                dict.set("entity_id", entity_id_to_i64(eid));
                arr.push(&dict);
            }
        }
        arr
    }

    #[func]
    pub fn get_board(&self, player: i64) -> Array<VarDictionary> {
        let Some(engine) = &self.engine else {
            return Array::new();
        };
        let Some(registry) = &self.registry else {
            return Array::new();
        };
        let p = player as PlayerId;
        let state = engine.state();
        let active = state.active_player;

        let mut arr = Array::new();
        for &eid in &state.players[p].board {
            let mut dict = VarDictionary::new();
            dict.set("entity_id", entity_id_to_i64(eid));

            if let Some(entity) = state.entities.get(&eid) {
                dict.set("card_id", GString::from(entity.card_id.as_str()));
                if let Some(card_def) = registry.get(&entity.card_id) {
                    dict.set("name", GString::from(card_def.name.as_str()));
                }
                if let Some(minion) = entity.as_minion() {
                    dict.set("attack", minion.attack as i64);
                    dict.set("health", minion.health as i64);
                    dict.set("max_health", minion.max_health as i64);
                    dict.set(
                        "summoning_sickness",
                        minion.summoning_sickness,
                    );
                    let mut kw_arr = Array::<GString>::new();
                    for kw in &minion.keywords {
                        kw_arr.push(&GString::from(keyword_str(kw)));
                    }
                    dict.set("keywords", kw_arr);

                    // can_attack: only meaningful for the active player's minions
                    let can = if p == active {
                        engine.can_entity_attack(p, eid).is_ok()
                    } else {
                        false
                    };
                    dict.set("can_attack", can);
                }
            }
            arr.push(&dict);
        }
        arr
    }

    #[func]
    pub fn get_hero(&self, player: i64) -> VarDictionary {
        let Some(engine) = &self.engine else {
            return VarDictionary::new();
        };
        let p = player as PlayerId;
        let state = engine.state();
        let hero = &state.players[p].hero;

        let mut dict = VarDictionary::new();
        dict.set("hp", hero.hp as i64);
        dict.set("max_hp", hero.max_hp as i64);
        dict.set("armor", hero.armor as i64);
        dict.set("entity_id", entity_id_to_i64(GameEngine::hero_entity_id(p)));

        // Weapon info
        if let Some(wep_id) = state.players[p].weapon {
            if let Some(entity) = state.entities.get(&wep_id) {
                if let EntityData::Weapon(w) = &entity.data {
                    let mut wep = VarDictionary::new();
                    wep.set("attack", w.attack as i64);
                    wep.set("durability", w.durability as i64);
                    wep.set("entity_id", entity_id_to_i64(wep_id));
                    dict.set("weapon", wep);
                }
            }
        }

        dict
    }

    #[func]
    pub fn get_mana(&self, player: i64) -> VarDictionary {
        let Some(engine) = &self.engine else {
            return VarDictionary::new();
        };
        let p = player as PlayerId;
        let state = engine.state();

        let mut dict = VarDictionary::new();
        dict.set("current", state.players[p].mana as i64);
        dict.set("max", state.players[p].mana_crystals as i64);
        dict
    }

    #[func]
    pub fn get_valid_targets(&self, hand_index: i64) -> Array<i64> {
        let Some(engine) = &self.engine else {
            return Array::new();
        };
        let player = engine.state().active_player;
        match engine.valid_play_targets_for_hand(player, hand_index as usize) {
            Some(targets) => {
                let mut arr = Array::new();
                for t in targets {
                    arr.push(entity_id_to_i64(t));
                }
                arr
            }
            None => Array::new(),
        }
    }

    #[func]
    pub fn can_attack(&self, entity_id: i64) -> bool {
        let Some(engine) = &self.engine else {
            return false;
        };
        let player = engine.state().active_player;
        engine
            .can_entity_attack(player, i64_to_entity_id(entity_id))
            .is_ok()
    }

    #[func]
    pub fn get_valid_attack_targets(&self, attacker_id: i64) -> Array<i64> {
        let Some(engine) = &self.engine else {
            return Array::new();
        };
        let player = engine.state().active_player;
        let targets =
            engine.valid_attack_targets(player, i64_to_entity_id(attacker_id));
        let mut arr = Array::new();
        for t in targets {
            arr.push(entity_id_to_i64(t));
        }
        arr
    }

    #[func]
    pub fn get_active_player(&self) -> i64 {
        self.engine
            .as_ref()
            .map_or(-1, |e| e.state().active_player as i64)
    }

    #[func]
    pub fn get_turn_number(&self) -> i64 {
        self.engine
            .as_ref()
            .map_or(0, |e| e.state().turn_number as i64)
    }

    #[func]
    pub fn is_game_over(&self) -> bool {
        self.engine
            .as_ref()
            .map_or(false, |e| e.state().game_over)
    }

    #[func]
    pub fn get_winner(&self) -> i64 {
        self.engine
            .as_ref()
            .and_then(|e| e.state().winner)
            .map_or(-1, |w| w as i64)
    }

    #[func]
    pub fn hero_entity_id(&self, player: i64) -> i64 {
        entity_id_to_i64(GameEngine::hero_entity_id(player as PlayerId))
    }

    #[func]
    pub fn needs_target(&self, hand_index: i64) -> bool {
        let Some(engine) = &self.engine else {
            return false;
        };
        let player = engine.state().active_player;
        engine
            .valid_play_targets_for_hand(player, hand_index as usize)
            .is_some()
    }

    #[func]
    pub fn get_deck_size(&self, player: i64) -> i64 {
        self.engine
            .as_ref()
            .map_or(0, |e| e.state().players[player as usize].deck.len() as i64)
    }
}

// --- Helper functions ---

fn entity_id_to_i64(eid: EntityId) -> i64 {
    eid as i64
}

fn i64_to_entity_id(val: i64) -> EntityId {
    val as EntityId // -1i64 as u64 == u64::MAX (P1 hero sentinel)
}

fn ok_result(events: &[Event]) -> VarDictionary {
    let mut dict = VarDictionary::new();
    dict.set("ok", true);
    dict.set("events", events_to_array(events));
    dict
}

fn error_result(msg: &str) -> VarDictionary {
    let mut dict = VarDictionary::new();
    dict.set("ok", false);
    dict.set("error", GString::from(msg));
    dict.set("events", Array::<VarDictionary>::new());
    dict
}

fn events_to_array(events: &[Event]) -> Array<VarDictionary> {
    let mut arr = Array::new();
    for event in events {
        arr.push(&event_to_dict(event));
    }
    arr
}

fn event_to_dict(event: &Event) -> VarDictionary {
    let mut dict = VarDictionary::new();
    match event {
        Event::GameStarted => {
            dict.set("event", GString::from("game_started"));
        }
        Event::TurnStarted {
            player,
            turn_number,
        } => {
            dict.set("event", GString::from("turn_started"));
            dict.set("player", *player as i64);
            dict.set("turn_number", *turn_number as i64);
        }
        Event::ManaGained {
            player,
            new_crystals,
        } => {
            dict.set("event", GString::from("mana_gained"));
            dict.set("player", *player as i64);
            dict.set("new_crystals", *new_crystals as i64);
        }
        Event::ManaRefilled { player, amount } => {
            dict.set("event", GString::from("mana_refilled"));
            dict.set("player", *player as i64);
            dict.set("amount", *amount as i64);
        }
        Event::CardDrawn {
            player,
            entity_id,
            card_id,
        } => {
            dict.set("event", GString::from("card_drawn"));
            dict.set("player", *player as i64);
            dict.set("entity_id", entity_id_to_i64(*entity_id));
            dict.set("card_id", GString::from(card_id.as_str()));
        }
        Event::CardBurned {
            player,
            entity_id,
            card_id,
        } => {
            dict.set("event", GString::from("card_burned"));
            dict.set("player", *player as i64);
            dict.set("entity_id", entity_id_to_i64(*entity_id));
            dict.set("card_id", GString::from(card_id.as_str()));
        }
        Event::FatigueDamage { player, damage } => {
            dict.set("event", GString::from("fatigue_damage"));
            dict.set("player", *player as i64);
            dict.set("damage", *damage as i64);
        }
        Event::CardPlayed {
            player,
            entity_id,
            card_id,
            hand_index,
        } => {
            dict.set("event", GString::from("card_played"));
            dict.set("player", *player as i64);
            dict.set("entity_id", entity_id_to_i64(*entity_id));
            dict.set("card_id", GString::from(card_id.as_str()));
            dict.set("hand_index", *hand_index as i64);
        }
        Event::ManaSpent {
            player,
            amount,
            remaining,
        } => {
            dict.set("event", GString::from("mana_spent"));
            dict.set("player", *player as i64);
            dict.set("amount", *amount as i64);
            dict.set("remaining", *remaining as i64);
        }
        Event::MinionSummoned {
            player,
            entity_id,
            position,
        } => {
            dict.set("event", GString::from("minion_summoned"));
            dict.set("player", *player as i64);
            dict.set("entity_id", entity_id_to_i64(*entity_id));
            dict.set("position", *position as i64);
        }
        Event::WeaponEquipped { player, entity_id } => {
            dict.set("event", GString::from("weapon_equipped"));
            dict.set("player", *player as i64);
            dict.set("entity_id", entity_id_to_i64(*entity_id));
        }
        Event::WeaponDestroyed { player, entity_id } => {
            dict.set("event", GString::from("weapon_destroyed"));
            dict.set("player", *player as i64);
            dict.set("entity_id", entity_id_to_i64(*entity_id));
        }
        Event::SpellCast { player, entity_id } => {
            dict.set("event", GString::from("spell_cast"));
            dict.set("player", *player as i64);
            dict.set("entity_id", entity_id_to_i64(*entity_id));
        }
        Event::AttackPerformed { attacker, defender } => {
            dict.set("event", GString::from("attack_performed"));
            dict.set("attacker", entity_id_to_i64(*attacker));
            dict.set("defender", entity_id_to_i64(*defender));
        }
        Event::DamageDealt {
            target,
            amount,
            source,
        } => {
            dict.set("event", GString::from("damage_dealt"));
            dict.set("target", entity_id_to_i64(*target));
            dict.set("amount", *amount as i64);
            if let Some(src) = source {
                dict.set("source", entity_id_to_i64(*src));
            }
        }
        Event::HeroDamaged {
            player,
            amount,
            new_hp,
        } => {
            dict.set("event", GString::from("hero_damaged"));
            dict.set("player", *player as i64);
            dict.set("amount", *amount as i64);
            dict.set("new_hp", *new_hp as i64);
        }
        Event::DivineShieldPopped { entity_id } => {
            dict.set("event", GString::from("divine_shield_popped"));
            dict.set("entity_id", entity_id_to_i64(*entity_id));
        }
        Event::MinionDied { entity_id, owner } => {
            dict.set("event", GString::from("minion_died"));
            dict.set("entity_id", entity_id_to_i64(*entity_id));
            dict.set("owner", *owner as i64);
        }
        Event::HeroDied { player } => {
            dict.set("event", GString::from("hero_died"));
            dict.set("player", *player as i64);
        }
        Event::GameOver { winner } => {
            dict.set("event", GString::from("game_over"));
            dict.set("winner", winner.map_or(-1i64, |w| w as i64));
        }
        Event::TurnEnded { player } => {
            dict.set("event", GString::from("turn_ended"));
            dict.set("player", *player as i64);
        }
    }
    dict
}

fn keyword_str(k: &Keyword) -> &'static str {
    match k {
        Keyword::Battlecry => "battlecry",
        Keyword::Deathrattle => "deathrattle",
        Keyword::Taunt => "taunt",
        Keyword::Charge => "charge",
        Keyword::DivineShield => "divine_shield",
    }
}

fn rarity_str(r: &hs_rules::card::Rarity) -> &'static str {
    match r {
        hs_rules::card::Rarity::Free => "free",
        hs_rules::card::Rarity::Common => "common",
        hs_rules::card::Rarity::Rare => "rare",
        hs_rules::card::Rarity::Epic => "epic",
        hs_rules::card::Rarity::Legendary => "legendary",
    }
}
