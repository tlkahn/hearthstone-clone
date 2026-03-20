use std::sync::Arc;

use rand::seq::SliceRandom;
use rand::RngCore;

use crate::action::Action;
use crate::card::{CardId, CardTypeData};
use crate::card_loader::CardRegistry;
use crate::entity::Entity;
use crate::error::GameError;
use crate::event::Event;
use crate::game_state::GameState;
use crate::types::*;

pub struct GameEngine {
    pub state: GameState,
    registry: Arc<CardRegistry>,
    rng: Box<dyn RngCore>,
}

impl GameEngine {
    pub fn new_game(
        registry: Arc<CardRegistry>,
        deck_p0: &[CardId],
        deck_p1: &[CardId],
        rng: Box<dyn RngCore>,
    ) -> (Self, Vec<Event>) {
        let mut engine = GameEngine {
            state: GameState::new(),
            registry,
            rng,
        };

        let mut events = vec![Event::GameStarted];

        // Create entities for both decks
        for &(player_id, deck_list) in &[(0usize, deck_p0), (1usize, deck_p1)] {
            for card_id in deck_list {
                if let Some(card_def) = engine.registry.get(card_id) {
                    let eid = engine.state.alloc_entity_id();
                    let entity = Entity::from_card_def(eid, card_def, player_id);
                    engine.state.entities.insert(eid, entity);
                    engine.state.players[player_id].deck.push(eid);
                }
            }
        }

        // Shuffle decks
        let rng = &mut engine.rng;
        engine.state.players[0].deck.shuffle(rng);
        let rng = &mut engine.rng;
        engine.state.players[1].deck.shuffle(rng);

        // Draw starting hands
        for _ in 0..STARTING_HAND_FIRST {
            events.extend(engine.draw_card(0));
        }
        for _ in 0..STARTING_HAND_SECOND {
            events.extend(engine.draw_card(1));
        }

        // Start turn 1 for player 0
        events.extend(engine.start_turn(0));

        (engine, events)
    }

    pub fn process_action(&mut self, action: Action) -> Result<Vec<Event>, GameError> {
        if self.state.game_over {
            return Err(GameError::GameOver);
        }

        let actor = action.player();
        if actor != self.state.active_player {
            return Err(GameError::NotYourTurn {
                active: self.state.active_player,
                actor,
            });
        }

        match action {
            Action::EndTurn { player } => self.handle_end_turn(player),
            Action::PlayCard {
                player,
                hand_index,
                position,
                target,
            } => self.handle_play_card(player, hand_index, position, target),
            Action::Attack {
                player,
                attacker,
                defender,
            } => self.handle_attack(player, attacker, defender),
        }
    }

    fn handle_end_turn(&mut self, player: PlayerId) -> Result<Vec<Event>, GameError> {
        let mut events = vec![Event::TurnEnded { player }];
        let next = self.state.opponent(player);
        events.extend(self.start_turn(next));
        Ok(events)
    }

    fn start_turn(&mut self, player: PlayerId) -> Vec<Event> {
        self.state.active_player = player;
        self.state.turn_number += 1;

        let mut events = Vec::new();

        events.push(Event::TurnStarted {
            player,
            turn_number: self.state.turn_number,
        });

        // Gain mana crystal
        if self.state.players[player].mana_crystals < MAX_MANA {
            self.state.players[player].mana_crystals += 1;
        }
        events.push(Event::ManaGained {
            player,
            new_crystals: self.state.players[player].mana_crystals,
        });

        // Refill mana
        self.state.players[player].mana = self.state.players[player].mana_crystals;
        events.push(Event::ManaRefilled {
            player,
            amount: self.state.players[player].mana,
        });

        // Clear summoning sickness and reset attacks for board minions
        for &eid in &self.state.players[player].board {
            if let Some(entity) = self.state.entities.get_mut(&eid) {
                if let Some(minion) = entity.as_minion_mut() {
                    minion.summoning_sickness = false;
                    minion.attacks_this_turn = 0;
                }
            }
        }

        // Draw card
        let draw_events = self.draw_card(player);
        events.extend(draw_events);

        // Check if hero died from fatigue
        if self.state.players[player].hero.is_dead() {
            events.extend(self.check_deaths_heroes());
        }

        events
    }

    pub(crate) fn draw_card(&mut self, player: PlayerId) -> Vec<Event> {
        let mut events = Vec::new();

        if self.state.players[player].deck.is_empty() {
            // Fatigue
            self.state.players[player].fatigue_counter += 1;
            let dmg = self.state.players[player].fatigue_counter;
            self.state.players[player].hero.hp -= dmg as i32;
            events.push(Event::FatigueDamage {
                player,
                damage: dmg,
            });
            if self.state.players[player].hero.is_dead() {
                events.push(Event::HeroDied { player });
                self.state.game_over = true;
                self.state.winner = Some(self.state.opponent(player));
                events.push(Event::GameOver {
                    winner: self.state.winner,
                });
            }
            return events;
        }

        let eid = self.state.players[player].deck.remove(0);
        let card_id = self
            .state
            .entities
            .get(&eid)
            .map(|e| e.card_id.clone())
            .unwrap_or_default();

        if self.state.players[player].hand.len() >= MAX_HAND_SIZE {
            // Burn the card
            events.push(Event::CardBurned {
                player,
                entity_id: eid,
                card_id,
            });
        } else {
            self.state.players[player].hand.push(eid);
            events.push(Event::CardDrawn {
                player,
                entity_id: eid,
                card_id,
            });
        }

        events
    }

    fn handle_play_card(
        &mut self,
        player: PlayerId,
        hand_index: usize,
        position: usize,
        target: Option<EntityId>,
    ) -> Result<Vec<Event>, GameError> {
        let hand_size = self.state.players[player].hand.len();
        if hand_index >= hand_size {
            return Err(GameError::InvalidHandIndex {
                index: hand_index,
                hand_size,
            });
        }

        let eid = self.state.players[player].hand[hand_index];
        let entity = self
            .state
            .entities
            .get(&eid)
            .ok_or(GameError::InvalidHandIndex {
                index: hand_index,
                hand_size,
            })?;

        let card_def = self
            .registry
            .get(&entity.card_id)
            .expect("card_id should exist in registry")
            .clone();
        let mana_cost = card_def.mana_cost;
        let card_type = card_def.card_type.clone();
        let card_id = entity.card_id.clone();
        let effects = card_def.effects.clone();
        let has_battlecry = card_def.keywords.contains(&crate::card::Keyword::Battlecry);

        // Check target requirement for spells / battlecries
        if Self::any_effect_requires_target(&effects) && target.is_none() {
            // Check if valid targets exist — if not, allow play without target
            let valid = self.valid_targets(&effects, player);
            if valid.as_ref().map_or(false, |t| !t.is_empty()) {
                return Err(GameError::TargetRequired);
            }
        }

        // Validate target if provided
        if let Some(tid) = target {
            if Self::any_effect_requires_target(&effects) {
                let valid = self.valid_targets(&effects, player);
                if let Some(ref targets) = valid {
                    if !targets.contains(&tid) {
                        return Err(GameError::InvalidTarget(tid));
                    }
                }
            }
        }

        // Check mana
        if self.state.players[player].mana < mana_cost {
            return Err(GameError::NotEnoughMana {
                available: self.state.players[player].mana,
                cost: mana_cost,
            });
        }

        let mut events = Vec::new();

        // Remove from hand
        self.state.players[player].hand.remove(hand_index);

        events.push(Event::CardPlayed {
            player,
            entity_id: eid,
            card_id: card_id.clone(),
            hand_index,
        });

        // Spend mana
        self.state.players[player].mana -= mana_cost;
        events.push(Event::ManaSpent {
            player,
            amount: mana_cost,
            remaining: self.state.players[player].mana,
        });

        match card_type {
            CardTypeData::Minion(_) => {
                // Check board space
                if self.state.players[player].board.len() >= MAX_BOARD_SIZE {
                    return Err(GameError::BoardFull);
                }

                let board_len = self.state.players[player].board.len();
                let insert_pos = position.min(board_len);
                self.state.players[player].board.insert(insert_pos, eid);

                events.push(Event::MinionSummoned {
                    player,
                    entity_id: eid,
                    position: insert_pos,
                });

                // Battlecry: only triggers when played from hand (not summoned by effect)
                if has_battlecry && !effects.is_empty() {
                    events.extend(self.execute_effects(&effects, player, target, 0));
                }
            }
            CardTypeData::Spell => {
                events.push(Event::SpellCast {
                    player,
                    entity_id: eid,
                });
                // Execute spell effects
                if !effects.is_empty() {
                    events.extend(self.execute_effects(&effects, player, target, 0));
                }
            }
            CardTypeData::Weapon(_) => {
                // Destroy existing weapon first
                if let Some(old_wep_id) = self.state.players[player].weapon.take() {
                    events.push(Event::WeaponDestroyed {
                        player,
                        entity_id: old_wep_id,
                    });
                }

                self.state.players[player].weapon = Some(eid);
                events.push(Event::WeaponEquipped {
                    player,
                    entity_id: eid,
                });
            }
        }

        Ok(events)
    }

    fn handle_attack(
        &mut self,
        player: PlayerId,
        attacker_id: EntityId,
        defender_id: EntityId,
    ) -> Result<Vec<Event>, GameError> {
        // Validate attacker is on player's board
        if !self.state.players[player].board.contains(&attacker_id) {
            return Err(GameError::InvalidAttacker(attacker_id));
        }

        // Validate defender belongs to opponent
        let opp = self.state.opponent(player);
        let _defender_is_hero = !self.state.entities.contains_key(&defender_id)
            || (!self.state.players[opp].board.contains(&defender_id)
                && !self.state.players[player].board.contains(&defender_id));

        // Check: cannot attack own minion
        if self.state.players[player].board.contains(&defender_id) {
            return Err(GameError::CannotAttackOwnMinion);
        }

        // Defender must be opponent's board minion or hero (entity_id 0 = p0 hero, entity_id special)
        // We'll use a convention: defender_id == 0 means hero of opponent
        // Actually, let's just check if defender is on opponent's board
        let attacking_hero = !self.state.players[opp].board.contains(&defender_id);

        // Summoning sickness / attack checks
        {
            let attacker = self
                .state
                .entities
                .get(&attacker_id)
                .ok_or(GameError::InvalidAttacker(attacker_id))?;
            let minion = attacker
                .as_minion()
                .ok_or(GameError::InvalidAttacker(attacker_id))?;

            if minion.summoning_sickness && !minion.keywords.contains(&crate::card::Keyword::Charge)
            {
                return Err(GameError::SummoningSickness);
            }
            if minion.attacks_this_turn >= 1 {
                return Err(GameError::AlreadyAttacked);
            }
        }

        // Taunt check
        let taunt_minions: Vec<EntityId> = self.state.players[opp]
            .board
            .iter()
            .filter(|&&eid| {
                self.state
                    .entities
                    .get(&eid)
                    .and_then(|e| e.as_minion())
                    .map_or(false, |m| m.keywords.contains(&crate::card::Keyword::Taunt))
            })
            .copied()
            .collect();

        if !taunt_minions.is_empty() && attacking_hero {
            return Err(GameError::MustAttackTaunt);
        }
        if !taunt_minions.is_empty() && !taunt_minions.contains(&defender_id) {
            return Err(GameError::MustAttackTaunt);
        }

        // Get attacker stats
        let attacker_attack = self
            .state
            .entities
            .get(&attacker_id)
            .and_then(|e| e.as_minion())
            .map(|m| m.attack)
            .unwrap();

        let mut events = vec![Event::AttackPerformed {
            attacker: attacker_id,
            defender: defender_id,
        }];

        // Mark attack used
        if let Some(m) = self
            .state
            .entities
            .get_mut(&attacker_id)
            .and_then(|e| e.as_minion_mut())
        {
            m.attacks_this_turn += 1;
        }

        if attacking_hero {
            // Minion attacks hero
            self.state.players[opp].hero.hp -= attacker_attack as i32;
            events.push(Event::DamageDealt {
                target: defender_id,
                amount: attacker_attack,
                source: Some(attacker_id),
            });
            events.push(Event::HeroDamaged {
                player: opp,
                amount: attacker_attack,
                new_hp: self.state.players[opp].hero.hp,
            });

            if self.state.players[opp].hero.is_dead() {
                events.push(Event::HeroDied { player: opp });
                self.state.game_over = true;
                self.state.winner = Some(player);
                events.push(Event::GameOver {
                    winner: Some(player),
                });
            }
        } else {
            // Minion vs minion combat
            let defender_attack = self
                .state
                .entities
                .get(&defender_id)
                .and_then(|e| e.as_minion())
                .map(|m| m.attack)
                .ok_or(GameError::InvalidDefender(defender_id))?;

            // Apply damage to defender
            let defender_has_shield = self
                .state
                .entities
                .get(&defender_id)
                .and_then(|e| e.as_minion())
                .map_or(false, |m| {
                    m.keywords.contains(&crate::card::Keyword::DivineShield)
                });

            if defender_has_shield {
                if let Some(m) = self
                    .state
                    .entities
                    .get_mut(&defender_id)
                    .and_then(|e| e.as_minion_mut())
                {
                    m.keywords.remove(&crate::card::Keyword::DivineShield);
                }
                events.push(Event::DivineShieldPopped {
                    entity_id: defender_id,
                });
            } else {
                if let Some(m) = self
                    .state
                    .entities
                    .get_mut(&defender_id)
                    .and_then(|e| e.as_minion_mut())
                {
                    m.health -= attacker_attack as i32;
                }
                events.push(Event::DamageDealt {
                    target: defender_id,
                    amount: attacker_attack,
                    source: Some(attacker_id),
                });
            }

            // Apply damage to attacker
            let attacker_has_shield = self
                .state
                .entities
                .get(&attacker_id)
                .and_then(|e| e.as_minion())
                .map_or(false, |m| {
                    m.keywords.contains(&crate::card::Keyword::DivineShield)
                });

            if attacker_has_shield {
                if let Some(m) = self
                    .state
                    .entities
                    .get_mut(&attacker_id)
                    .and_then(|e| e.as_minion_mut())
                {
                    m.keywords.remove(&crate::card::Keyword::DivineShield);
                }
                events.push(Event::DivineShieldPopped {
                    entity_id: attacker_id,
                });
            } else {
                if let Some(m) = self
                    .state
                    .entities
                    .get_mut(&attacker_id)
                    .and_then(|e| e.as_minion_mut())
                {
                    m.health -= defender_attack as i32;
                }
                events.push(Event::DamageDealt {
                    target: attacker_id,
                    amount: defender_attack,
                    source: Some(defender_id),
                });
            }

            // Process deaths
            events.extend(self.process_minion_deaths());
        }

        Ok(events)
    }

    fn process_minion_deaths(&mut self) -> Vec<Event> {
        self.process_deaths_with_deathrattles(0)
    }

    fn check_deaths_heroes(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        for p in 0..2 {
            if self.state.players[p].hero.is_dead() && !self.state.game_over {
                events.push(Event::HeroDied { player: p });
                self.state.game_over = true;
                self.state.winner = Some(self.state.opponent(p));
                events.push(Event::GameOver {
                    winner: self.state.winner,
                });
            }
        }
        events
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn registry(&self) -> &CardRegistry {
        &self.registry
    }

    /// Sentinel EntityId used to represent the opponent's hero as an attack target.
    pub const HERO_ENTITY_ID: EntityId = 0;

    // --- Query methods for the bridge (Phase 1a) ---

    /// Check whether an entity can attack this turn. Returns Ok(()) or the specific error.
    pub fn can_entity_attack(&self, player: PlayerId, entity_id: EntityId) -> Result<(), GameError> {
        if !self.state.players[player].board.contains(&entity_id) {
            return Err(GameError::InvalidAttacker(entity_id));
        }
        let entity = self
            .state
            .entities
            .get(&entity_id)
            .ok_or(GameError::InvalidAttacker(entity_id))?;
        let minion = entity
            .as_minion()
            .ok_or(GameError::InvalidAttacker(entity_id))?;

        if minion.summoning_sickness
            && !minion.keywords.contains(&crate::card::Keyword::Charge)
        {
            return Err(GameError::SummoningSickness);
        }
        if minion.attacks_this_turn >= 1 {
            return Err(GameError::AlreadyAttacked);
        }
        Ok(())
    }

    /// Returns all valid attack targets for a given attacker (opponent minions + hero,
    /// filtered by taunt).
    pub fn valid_attack_targets(
        &self,
        player: PlayerId,
        _attacker_id: EntityId,
    ) -> Vec<EntityId> {
        let opp = self.state.opponent(player);

        let taunt_minions: Vec<EntityId> = self.state.players[opp]
            .board
            .iter()
            .filter(|&&eid| {
                self.state
                    .entities
                    .get(&eid)
                    .and_then(|e| e.as_minion())
                    .map_or(false, |m| {
                        m.keywords.contains(&crate::card::Keyword::Taunt)
                    })
            })
            .copied()
            .collect();

        if !taunt_minions.is_empty() {
            return taunt_minions;
        }

        // No taunt — all opponent minions + opponent hero
        let mut targets: Vec<EntityId> = self.state.players[opp].board.clone();
        targets.push(Self::hero_entity_id(opp));
        targets
    }

    /// Check if a card at `hand_index` is playable (enough mana, board not full for minions).
    pub fn is_card_playable(&self, player: PlayerId, hand_index: usize) -> bool {
        let hand = &self.state.players[player].hand;
        if hand_index >= hand.len() {
            return false;
        }
        let eid = hand[hand_index];
        let entity = match self.state.entities.get(&eid) {
            Some(e) => e,
            None => return false,
        };
        let card_def = match self.registry.get(&entity.card_id) {
            Some(c) => c,
            None => return false,
        };
        if card_def.mana_cost > self.state.players[player].mana {
            return false;
        }
        if matches!(card_def.card_type, CardTypeData::Minion(_))
            && self.state.players[player].board.len() >= MAX_BOARD_SIZE
        {
            return false;
        }
        true
    }

    /// For a card at `hand_index`, returns Some(targets) if it needs a target, None otherwise.
    pub fn valid_play_targets_for_hand(
        &self,
        player: PlayerId,
        hand_index: usize,
    ) -> Option<Vec<EntityId>> {
        let hand = &self.state.players[player].hand;
        if hand_index >= hand.len() {
            return None;
        }
        let eid = hand[hand_index];
        let entity = self.state.entities.get(&eid)?;
        let card_def = self.registry.get(&entity.card_id)?;
        if Self::any_effect_requires_target(&card_def.effects) {
            self.valid_targets(&card_def.effects, player)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::*;
    use crate::card_loader::CardRegistry;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use std::sync::Arc;

    fn test_registry() -> Arc<CardRegistry> {
        // Build a registry with a few test cards using temp files
        let dir = tempfile::TempDir::new().unwrap();
        let ron = r#"
            CardSet(
                name: "Test",
                cards: [
                    CardDef(
                        id: "test_minion_2_3",
                        name: "Test Minion",
                        mana_cost: 2,
                        card_type: Minion(MinionStats(attack: 2, health: 3)),
                        rarity: Free,
                        keywords: [],
                        text: "",
                        art: "test.png",
                    ),
                    CardDef(
                        id: "test_minion_3_2",
                        name: "Strong Minion",
                        mana_cost: 3,
                        card_type: Minion(MinionStats(attack: 3, health: 2)),
                        rarity: Free,
                        keywords: [],
                        text: "",
                        art: "test.png",
                    ),
                    CardDef(
                        id: "test_taunt_1_5",
                        name: "Taunt Wall",
                        mana_cost: 3,
                        card_type: Minion(MinionStats(attack: 1, health: 5)),
                        rarity: Free,
                        keywords: [Taunt],
                        text: "Taunt",
                        art: "test.png",
                    ),
                    CardDef(
                        id: "test_charge_4_2",
                        name: "Charger",
                        mana_cost: 4,
                        card_type: Minion(MinionStats(attack: 4, health: 2)),
                        rarity: Free,
                        keywords: [Charge],
                        text: "Charge",
                        art: "test.png",
                    ),
                    CardDef(
                        id: "test_shield_2_2",
                        name: "Shielded",
                        mana_cost: 2,
                        card_type: Minion(MinionStats(attack: 2, health: 2)),
                        rarity: Free,
                        keywords: [DivineShield],
                        text: "Divine Shield",
                        art: "test.png",
                    ),
                    CardDef(
                        id: "test_spell",
                        name: "Test Spell",
                        mana_cost: 1,
                        card_type: Spell,
                        rarity: Free,
                        keywords: [],
                        text: "Do nothing",
                        art: "test.png",
                    ),
                    CardDef(
                        id: "test_weapon_3_2",
                        name: "Test Weapon",
                        mana_cost: 3,
                        card_type: Weapon(WeaponStats(attack: 3, durability: 2)),
                        rarity: Free,
                        keywords: [],
                        text: "",
                        art: "test.png",
                    ),
                    CardDef(
                        id: "test_weapon_2_1",
                        name: "Small Weapon",
                        mana_cost: 2,
                        card_type: Weapon(WeaponStats(attack: 2, durability: 1)),
                        rarity: Free,
                        keywords: [],
                        text: "",
                        art: "test.png",
                    ),
                    CardDef(
                        id: "test_minion_1_1",
                        name: "Tiny Minion",
                        mana_cost: 1,
                        card_type: Minion(MinionStats(attack: 1, health: 1)),
                        rarity: Free,
                        keywords: [],
                        text: "",
                        art: "test.png",
                    ),
                    CardDef(
                        id: "test_fireball",
                        name: "Fireball",
                        mana_cost: 4,
                        card_type: Spell,
                        rarity: Free,
                        keywords: [],
                        text: "Deal 6 damage.",
                        art: "test.png",
                        effects: [DealDamage(amount: 6, target: PlayerChoice(EnemyCharacter))],
                    ),
                    CardDef(
                        id: "test_leeroy",
                        name: "Leeroy Jenkins",
                        mana_cost: 5,
                        card_type: Minion(MinionStats(attack: 6, health: 2)),
                        rarity: Legendary,
                        keywords: [Charge, Battlecry],
                        text: "Charge. Battlecry: Summon two 1/1 Whelps for your opponent.",
                        art: "test.png",
                        effects: [Summon(card_id: "test_token_whelp", count: 2, for_opponent: true)],
                    ),
                    CardDef(
                        id: "test_token_whelp",
                        name: "Whelp",
                        mana_cost: 1,
                        card_type: Minion(MinionStats(attack: 1, health: 1)),
                        rarity: Free,
                        keywords: [],
                        text: "",
                        art: "test.png",
                    ),
                    CardDef(
                        id: "test_heal_spell",
                        name: "Healing Touch",
                        mana_cost: 3,
                        card_type: Spell,
                        rarity: Free,
                        keywords: [],
                        text: "Heal 8.",
                        art: "test.png",
                        effects: [Heal(amount: 8, target: PlayerChoice(AnyCharacter))],
                    ),
                    CardDef(
                        id: "test_buff_spell",
                        name: "Blessing",
                        mana_cost: 1,
                        card_type: Spell,
                        rarity: Free,
                        keywords: [],
                        text: "+2/+2",
                        art: "test.png",
                        effects: [BuffMinion(attack: 2, health: 2, target: PlayerChoice(AnyMinion))],
                    ),
                    CardDef(
                        id: "test_draw_spell",
                        name: "Arcane Intellect",
                        mana_cost: 3,
                        card_type: Spell,
                        rarity: Free,
                        keywords: [],
                        text: "Draw 2 cards.",
                        art: "test.png",
                        effects: [DrawCards(count: 2)],
                    ),
                    CardDef(
                        id: "test_destroy_spell",
                        name: "Assassinate",
                        mana_cost: 5,
                        card_type: Spell,
                        rarity: Free,
                        keywords: [],
                        text: "Destroy a minion.",
                        art: "test.png",
                        effects: [DestroyMinion(target: PlayerChoice(AnyMinion))],
                    ),
                    CardDef(
                        id: "test_deathrattle_minion",
                        name: "Loot Hoarder",
                        mana_cost: 2,
                        card_type: Minion(MinionStats(attack: 2, health: 1)),
                        rarity: Free,
                        keywords: [Deathrattle],
                        text: "Deathrattle: Draw a card.",
                        art: "test.png",
                        effects: [DrawCards(count: 1)],
                    ),
                ],
            )
        "#;
        std::fs::write(dir.path().join("test.ron"), ron).unwrap();
        Arc::new(CardRegistry::load_from_directory(dir.path()).unwrap())
    }

    fn make_deck(card_id: &str, count: usize) -> Vec<CardId> {
        vec![card_id.to_string(); count]
    }

    fn fixed_rng() -> Box<dyn RngCore> {
        Box::new(StdRng::seed_from_u64(42))
    }

    // ==================== Phase 1 Tests ====================

    #[test]
    fn game_creates_with_correct_initial_state() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (engine, _events) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());
        let s = engine.state();

        assert_eq!(s.players[0].hero.hp, STARTING_HP);
        assert_eq!(s.players[1].hero.hp, STARTING_HP);
        // After drawing starting hands: P0 has 3, P1 has 4
        // After start_turn for P0: draws 1 more → P0 has 4
        assert_eq!(s.players[0].hand.len(), STARTING_HAND_FIRST + 1); // +1 from turn start draw
        assert_eq!(s.players[1].hand.len(), STARTING_HAND_SECOND);
        assert_eq!(
            s.players[0].deck.len(),
            30 - STARTING_HAND_FIRST - 1 // -1 from turn start draw
        );
        assert_eq!(s.players[1].deck.len(), 30 - STARTING_HAND_SECOND);
    }

    #[test]
    fn starting_hands_drawn() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (_engine, events) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        // Count CardDrawn events per player
        let p0_draws = events
            .iter()
            .filter(|e| matches!(e, Event::CardDrawn { player: 0, .. }))
            .count();
        let p1_draws = events
            .iter()
            .filter(|e| matches!(e, Event::CardDrawn { player: 1, .. }))
            .count();

        // P0 gets 3 starting + 1 turn start = 4 draws
        assert_eq!(p0_draws, STARTING_HAND_FIRST + 1);
        assert_eq!(p1_draws, STARTING_HAND_SECOND);
    }

    #[test]
    fn deck_shuffling_is_deterministic() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 10);

        let (e1, _) =
            GameEngine::new_game(reg.clone(), &deck, &deck, Box::new(StdRng::seed_from_u64(42)));
        let (e2, _) =
            GameEngine::new_game(reg, &deck, &deck, Box::new(StdRng::seed_from_u64(42)));

        assert_eq!(e1.state().players[0].deck, e2.state().players[0].deck);
        assert_eq!(e1.state().players[1].deck, e2.state().players[1].deck);
    }

    #[test]
    fn turn_start_gains_mana_crystal() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        // After new_game, turn 1 started for P0 → 1 crystal
        assert_eq!(engine.state().players[0].mana_crystals, 1);
        assert_eq!(engine.state().players[0].mana, 1);

        // End turn → P1 gets turn, should have 1 crystal
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        assert_eq!(engine.state().players[1].mana_crystals, 1);

        // End turn → P0 gets turn 2 → 2 crystals
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();
        assert_eq!(engine.state().players[0].mana_crystals, 2);
        assert_eq!(engine.state().players[0].mana, 2);
    }

    #[test]
    fn mana_refills_to_crystal_count() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        // Spend some mana by playing a card (cost 2, have 1 on turn 1)
        // Skip to turn 3 (3 mana) to play a 2-cost card
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();
        // P0 turn 2, 2 mana
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();
        // P0 turn 3, 3 mana

        // Play a 2-cost card
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        assert_eq!(engine.state().players[0].mana, 1);

        // End turn and come back → mana refilled to 4
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();
        assert_eq!(engine.state().players[0].mana_crystals, 4);
        assert_eq!(engine.state().players[0].mana, 4);
    }

    #[test]
    fn turn_start_draws_one_card() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        let hand_before = engine.state().players[1].hand.len();
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        assert_eq!(engine.state().players[1].hand.len(), hand_before + 1);
    }

    #[test]
    fn full_hand_burns_card() {
        let reg = test_registry();
        // Give P1 a large deck so they draw many cards
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        // P1 starts with 4 cards. Pass turns until P1 has 10 cards (full hand)
        // Each P1 turn start draws 1. Need 6 more draws to reach 10.
        for _ in 0..6 {
            engine
                .process_action(Action::EndTurn { player: 0 })
                .unwrap();
            engine
                .process_action(Action::EndTurn { player: 1 })
                .unwrap();
        }

        // P0's turn. P1 has 4 + 6 = 10 cards
        assert_eq!(engine.state().players[1].hand.len(), MAX_HAND_SIZE);

        // End turn → P1 draws, should burn
        let deck_before = engine.state().players[1].deck.len();
        let events = engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();

        // Hand stays at 10
        assert_eq!(engine.state().players[1].hand.len(), MAX_HAND_SIZE);
        // Deck decreased by 1
        assert_eq!(engine.state().players[1].deck.len(), deck_before - 1);
        // Should have a CardBurned event
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::CardBurned { player: 1, .. })));
    }

    #[test]
    fn fatigue_deals_incrementing_damage() {
        let reg = test_registry();
        // Small decks so we fatigue quickly
        let deck = make_deck("test_minion_2_3", 4);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        // P0 drew 3+1=4, deck now empty. P1 drew 4, deck now empty.
        assert_eq!(engine.state().players[0].deck.len(), 0);
        assert_eq!(engine.state().players[1].deck.len(), 0);

        // End P0's turn → P1 takes fatigue 1
        let events = engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::FatigueDamage { player: 1, damage: 1 })));
        assert_eq!(engine.state().players[1].hero.hp, STARTING_HP - 1);

        // End P1's turn → P0 takes fatigue (P0 already has fatigue_counter=0, so this is 1)
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();
        assert_eq!(engine.state().players[0].hero.hp, STARTING_HP - 1);

        // End P0's turn → P1 takes fatigue 2
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        assert_eq!(engine.state().players[1].hero.hp, STARTING_HP - 1 - 2);
        assert_eq!(engine.state().players[1].fatigue_counter, 2);
    }

    #[test]
    fn hero_at_zero_hp_game_over() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 4);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        // Manually set hero HP low
        engine.state.players[1].hero.hp = 1;

        // End turn → P1 draws fatigue (1 damage)
        let events = engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();

        assert!(engine.state().game_over);
        assert_eq!(engine.state().winner, Some(0));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::HeroDied { player: 1 })));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::GameOver { winner: Some(0) })));
    }

    #[test]
    fn end_turn_switches_active_player() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        assert_eq!(engine.state().active_player, 0);
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        assert_eq!(engine.state().active_player, 1);
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();
        assert_eq!(engine.state().active_player, 0);
    }

    #[test]
    fn action_rejected_when_not_your_turn() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        let result = engine.process_action(Action::EndTurn { player: 1 });
        assert!(matches!(result, Err(GameError::NotYourTurn { .. })));
    }

    #[test]
    fn action_rejected_when_game_over() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.game_over = true;
        let result = engine.process_action(Action::EndTurn { player: 0 });
        assert!(matches!(result, Err(GameError::GameOver)));
    }

    #[test]
    fn mana_caps_at_ten() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        // Pass 20 turns to go well past 10 crystals
        for _ in 0..20 {
            engine
                .process_action(Action::EndTurn {
                    player: engine.state().active_player,
                })
                .unwrap();
        }

        assert!(engine.state().players[0].mana_crystals <= MAX_MANA);
        assert!(engine.state().players[1].mana_crystals <= MAX_MANA);
        assert!(engine.state().players[0].mana <= MAX_MANA);
        assert!(engine.state().players[1].mana <= MAX_MANA);
    }

    #[test]
    fn end_turn_produces_correct_event_sequence() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        let events = engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();

        // Should contain: TurnEnded(0), TurnStarted(1), ManaGained(1), ManaRefilled(1), CardDrawn(1)
        assert!(matches!(events[0], Event::TurnEnded { player: 0 }));
        assert!(matches!(
            events[1],
            Event::TurnStarted {
                player: 1,
                turn_number: 2
            }
        ));
        assert!(matches!(events[2], Event::ManaGained { player: 1, .. }));
        assert!(matches!(events[3], Event::ManaRefilled { player: 1, .. }));
        assert!(matches!(events[4], Event::CardDrawn { player: 1, .. }));
    }

    // ==================== Phase 2 Tests ====================

    #[test]
    fn play_minion_moves_from_hand_to_board() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        // Advance to turn where P0 has enough mana (need 2 for test_minion_2_3)
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();
        // P0 turn 2, 2 mana

        let hand_before = engine.state().players[0].hand.len();
        let board_before = engine.state().players[0].board.len();
        let eid = engine.state().players[0].hand[0];

        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();

        assert_eq!(engine.state().players[0].hand.len(), hand_before - 1);
        assert_eq!(engine.state().players[0].board.len(), board_before + 1);
        assert!(engine.state().players[0].board.contains(&eid));
    }

    #[test]
    fn mana_deducted_correctly() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        // Get to 2 mana
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        assert_eq!(engine.state().players[0].mana, 2);
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        assert_eq!(engine.state().players[0].mana, 0);
    }

    #[test]
    fn not_enough_mana_error() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        // Turn 1: 1 mana, card costs 2
        let result = engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 0,
            position: 0,
            target: None,
        });

        assert!(matches!(result, Err(GameError::NotEnoughMana { .. })));
    }

    #[test]
    fn board_full_error() {
        let reg = test_registry();
        let deck = make_deck("test_minion_1_1", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        // Give P0 lots of mana and ensure enough cards in hand
        engine.state.players[0].mana = 20;
        engine.state.players[0].mana_crystals = 10;

        // Move cards from deck to hand to ensure we have enough
        while engine.state.players[0].hand.len() < 8 {
            if let Some(eid) = engine.state.players[0].deck.pop() {
                engine.state.players[0].hand.push(eid);
            }
        }

        // Fill board with 7 minions
        for _ in 0..7 {
            engine
                .process_action(Action::PlayCard {
                    player: 0,
                    hand_index: 0,
                    position: 0,
                    target: None,
                })
                .unwrap();
        }

        assert_eq!(engine.state().players[0].board.len(), MAX_BOARD_SIZE);

        // 8th should fail
        let result = engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 0,
            position: 0,
            target: None,
        });
        assert!(matches!(result, Err(GameError::BoardFull)));
    }

    #[test]
    fn summoning_sickness_on_new_minion() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();

        let eid = engine.state().players[0].board[0];
        let minion = engine.state().entities.get(&eid).unwrap().as_minion().unwrap();
        assert!(minion.summoning_sickness);
    }

    #[test]
    fn position_insertion() {
        let reg = test_registry();
        let deck = make_deck("test_minion_1_1", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 20;

        // Play three minions at different positions
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let first = engine.state().players[0].board[0];

        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let second = engine.state().players[0].board[0]; // inserted at position 0

        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 1,
                target: None,
            })
            .unwrap();
        let third = engine.state().players[0].board[1]; // inserted at position 1

        // Board order should be: second, third, first
        assert_eq!(engine.state().players[0].board, vec![second, third, first]);
    }

    #[test]
    fn spell_removes_from_hand() {
        let reg = test_registry();
        let deck = make_deck("test_spell", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        let hand_before = engine.state().players[0].hand.len();
        let events = engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();

        assert_eq!(engine.state().players[0].hand.len(), hand_before - 1);
        assert!(events.iter().any(|e| matches!(e, Event::SpellCast { .. })));
    }

    #[test]
    fn weapon_equips() {
        let reg = test_registry();
        let deck = make_deck("test_weapon_3_2", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;
        let events = engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();

        assert!(engine.state().players[0].weapon.is_some());
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::WeaponEquipped { .. })));
    }

    #[test]
    fn new_weapon_destroys_existing() {
        let reg = test_registry();
        let deck = make_deck("test_weapon_3_2", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 20;

        // Equip first weapon
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let first_wep = engine.state().players[0].weapon.unwrap();

        // Equip second weapon
        let events = engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();

        assert_ne!(engine.state().players[0].weapon.unwrap(), first_wep);
        assert!(events.iter().any(|e| matches!(
            e,
            Event::WeaponDestroyed {
                entity_id,
                ..
            } if *entity_id == first_wep
        )));
    }

    #[test]
    fn invalid_hand_index_error() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        let result = engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 99,
            position: 0,
            target: None,
        });
        assert!(matches!(result, Err(GameError::InvalidHandIndex { .. })));
    }

    #[test]
    fn play_card_correct_event_sequence() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;
        let events = engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();

        assert!(matches!(events[0], Event::CardPlayed { .. }));
        assert!(matches!(events[1], Event::ManaSpent { .. }));
        assert!(matches!(events[2], Event::MinionSummoned { .. }));
    }

    #[test]
    fn multiple_cards_per_turn() {
        let reg = test_registry();
        let deck = make_deck("test_minion_1_1", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;

        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();

        assert_eq!(engine.state().players[0].board.len(), 2);
        assert_eq!(engine.state().players[0].mana, 8);
    }

    // ==================== Phase 3 Tests ====================

    #[test]
    fn minion_attacks_minion_mutual_damage() {
        let reg = test_registry();
        let deck_p0 = make_deck("test_minion_2_3", 30);
        let deck_p1 = make_deck("test_minion_3_2", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck_p0, &deck_p1, fixed_rng());

        engine.state.players[0].mana = 10;
        engine.state.players[1].mana = 10;

        // P0 plays a 2/3
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let attacker_id = engine.state().players[0].board[0];

        // End turn so P1 can play
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine.state.players[1].mana = 10;

        // P1 plays a 3/2
        engine
            .process_action(Action::PlayCard {
                player: 1,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let defender_id = engine.state().players[1].board[0];

        // End turn so P0 can attack (clears summoning sickness)
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        // P0's minion should no longer have summoning sickness
        engine
            .process_action(Action::Attack {
                player: 0,
                attacker: attacker_id,
                defender: defender_id,
            })
            .unwrap();

        // 2/3 attacks 3/2: attacker takes 3 dmg (dies), defender takes 2 dmg (dies)
        // Both should be dead and in graveyard
        assert!(!engine.state().players[0].board.contains(&attacker_id));
        assert!(!engine.state().players[1].board.contains(&defender_id));
    }

    #[test]
    fn minion_attacks_hero() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;

        // Play minion
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let attacker_id = engine.state().players[0].board[0];

        // End turn twice to clear summoning sickness
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        // Attack hero (use HERO_ENTITY_ID)
        engine
            .process_action(Action::Attack {
                player: 0,
                attacker: attacker_id,
                defender: GameEngine::HERO_ENTITY_ID,
            })
            .unwrap();

        assert_eq!(engine.state().players[1].hero.hp, STARTING_HP - 2);
    }

    #[test]
    fn summoning_sick_cannot_attack() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;

        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let attacker_id = engine.state().players[0].board[0];

        let result = engine.process_action(Action::Attack {
            player: 0,
            attacker: attacker_id,
            defender: GameEngine::HERO_ENTITY_ID,
        });

        assert!(matches!(result, Err(GameError::SummoningSickness)));
    }

    #[test]
    fn can_attack_after_surviving_one_turn() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let attacker_id = engine.state().players[0].board[0];

        // Pass turns to clear sickness
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        let result = engine.process_action(Action::Attack {
            player: 0,
            attacker: attacker_id,
            defender: GameEngine::HERO_ENTITY_ID,
        });
        assert!(result.is_ok());
    }

    #[test]
    fn cannot_attack_twice_per_turn() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let attacker_id = engine.state().players[0].board[0];

        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        // First attack
        engine
            .process_action(Action::Attack {
                player: 0,
                attacker: attacker_id,
                defender: GameEngine::HERO_ENTITY_ID,
            })
            .unwrap();

        // Second attack
        let result = engine.process_action(Action::Attack {
            player: 0,
            attacker: attacker_id,
            defender: GameEngine::HERO_ENTITY_ID,
        });
        assert!(matches!(result, Err(GameError::AlreadyAttacked)));
    }

    #[test]
    fn attacks_reset_each_turn() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let attacker_id = engine.state().players[0].board[0];

        // Clear sickness
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        // Attack once
        engine
            .process_action(Action::Attack {
                player: 0,
                attacker: attacker_id,
                defender: GameEngine::HERO_ENTITY_ID,
            })
            .unwrap();

        // Next turn
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        // Should be able to attack again
        let result = engine.process_action(Action::Attack {
            player: 0,
            attacker: attacker_id,
            defender: GameEngine::HERO_ENTITY_ID,
        });
        assert!(result.is_ok());
    }

    #[test]
    fn hero_dies_game_over() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let attacker_id = engine.state().players[0].board[0];

        // Clear sickness
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        // Set hero HP low
        engine.state.players[1].hero.hp = 2;

        let events = engine
            .process_action(Action::Attack {
                player: 0,
                attacker: attacker_id,
                defender: GameEngine::HERO_ENTITY_ID,
            })
            .unwrap();

        assert!(engine.state().game_over);
        assert_eq!(engine.state().winner, Some(0));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::GameOver { winner: Some(0) })));
    }

    #[test]
    fn cannot_attack_own_minion() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;

        // Play two minions
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();

        let a = engine.state().players[0].board[0];
        let b = engine.state().players[0].board[1];

        // Clear sickness
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        let result = engine.process_action(Action::Attack {
            player: 0,
            attacker: a,
            defender: b,
        });
        assert!(matches!(result, Err(GameError::CannotAttackOwnMinion)));
    }

    #[test]
    fn dead_minions_go_to_graveyard() {
        let reg = test_registry();
        let deck_p0 = make_deck("test_minion_2_3", 30);
        let deck_p1 = make_deck("test_minion_1_1", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck_p0, &deck_p1, fixed_rng());

        engine.state.players[0].mana = 10;
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let attacker_id = engine.state().players[0].board[0];

        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine.state.players[1].mana = 10;

        engine
            .process_action(Action::PlayCard {
                player: 1,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let defender_id = engine.state().players[1].board[0];

        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        // 2/3 attacks 1/1: defender dies, attacker survives with 2/2
        let events = engine
            .process_action(Action::Attack {
                player: 0,
                attacker: attacker_id,
                defender: defender_id,
            })
            .unwrap();

        assert!(engine.state().players[0].board.contains(&attacker_id));
        assert!(!engine.state().players[1].board.contains(&defender_id));
        assert!(engine.state().players[1].graveyard.contains(&defender_id));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::MinionDied { entity_id, .. } if *entity_id == defender_id)));
    }

    // ==================== Phase 4 Tests ====================

    #[test]
    fn taunt_must_be_attacked_first() {
        let reg = test_registry();
        let deck_p0 = make_deck("test_minion_2_3", 30);
        let deck_p1 = make_deck("test_taunt_1_5", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck_p0, &deck_p1, fixed_rng());

        engine.state.players[0].mana = 10;
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let attacker_id = engine.state().players[0].board[0];

        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine.state.players[1].mana = 10;
        engine
            .process_action(Action::PlayCard {
                player: 1,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();

        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        // Try to attack hero — should be blocked by taunt
        let result = engine.process_action(Action::Attack {
            player: 0,
            attacker: attacker_id,
            defender: GameEngine::HERO_ENTITY_ID,
        });
        assert!(matches!(result, Err(GameError::MustAttackTaunt)));
    }

    #[test]
    fn can_attack_hero_when_no_taunt() {
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let attacker_id = engine.state().players[0].board[0];

        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        let result = engine.process_action(Action::Attack {
            player: 0,
            attacker: attacker_id,
            defender: GameEngine::HERO_ENTITY_ID,
        });
        assert!(result.is_ok());
    }

    #[test]
    fn cannot_attack_hero_past_taunt() {
        let reg = test_registry();
        let deck_p0 = make_deck("test_minion_2_3", 30);
        // P1 has a taunt and a non-taunt
        let (mut engine, _) = GameEngine::new_game(reg.clone(), &deck_p0, &deck_p0, fixed_rng());

        engine.state.players[0].mana = 10;
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let attacker_id = engine.state().players[0].board[0];

        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine.state.players[1].mana = 10;

        // Manually place a taunt minion on P1's board
        let taunt_eid = engine.state.alloc_entity_id();
        let taunt_card = reg.get("test_taunt_1_5").unwrap();
        let taunt_entity = Entity::from_card_def(taunt_eid, taunt_card, 1);
        engine.state.entities.insert(taunt_eid, taunt_entity);
        engine.state.players[1].board.push(taunt_eid);

        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        let result = engine.process_action(Action::Attack {
            player: 0,
            attacker: attacker_id,
            defender: GameEngine::HERO_ENTITY_ID,
        });
        assert!(matches!(result, Err(GameError::MustAttackTaunt)));
    }

    #[test]
    fn can_attack_hero_after_taunt_dies() {
        let reg = test_registry();
        let deck_p0 = make_deck("test_minion_2_3", 30);
        let deck_p1 = make_deck("test_taunt_1_5", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck_p0, &deck_p1, fixed_rng());

        engine.state.players[0].mana = 10;
        // Play two minions
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();

        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine.state.players[1].mana = 10;

        // P1 plays taunt (1/5)
        engine
            .process_action(Action::PlayCard {
                player: 1,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let taunt_id = engine.state().players[1].board[0];

        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        // Kill the taunt: set its health to 1 so our 2-attack minion kills it
        if let Some(m) = engine
            .state
            .entities
            .get_mut(&taunt_id)
            .and_then(|e| e.as_minion_mut())
        {
            m.health = 1;
        }

        let a1 = engine.state().players[0].board[0];
        let a2 = engine.state().players[0].board[1];

        // Kill taunt with first minion
        engine
            .process_action(Action::Attack {
                player: 0,
                attacker: a1,
                defender: taunt_id,
            })
            .unwrap();

        // Now second minion can attack hero
        let result = engine.process_action(Action::Attack {
            player: 0,
            attacker: a2,
            defender: GameEngine::HERO_ENTITY_ID,
        });
        assert!(result.is_ok());
    }

    #[test]
    fn charge_can_attack_immediately() {
        let reg = test_registry();
        let deck = make_deck("test_charge_4_2", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let charger = engine.state().players[0].board[0];

        // Should be able to attack immediately despite summoning sickness
        let result = engine.process_action(Action::Attack {
            player: 0,
            attacker: charger,
            defender: GameEngine::HERO_ENTITY_ID,
        });
        assert!(result.is_ok());
        assert_eq!(engine.state().players[1].hero.hp, STARTING_HP - 4);
    }

    #[test]
    fn divine_shield_absorbs_first_hit() {
        let reg = test_registry();
        let deck_p0 = make_deck("test_minion_2_3", 30);
        let deck_p1 = make_deck("test_shield_2_2", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck_p0, &deck_p1, fixed_rng());

        engine.state.players[0].mana = 10;
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let attacker_id = engine.state().players[0].board[0];

        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine.state.players[1].mana = 10;
        engine
            .process_action(Action::PlayCard {
                player: 1,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let defender_id = engine.state().players[1].board[0];

        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        let events = engine
            .process_action(Action::Attack {
                player: 0,
                attacker: attacker_id,
                defender: defender_id,
            })
            .unwrap();

        // Shield absorbed: defender (2/2 divine shield) takes no damage
        let def = engine
            .state()
            .entities
            .get(&defender_id)
            .unwrap()
            .as_minion()
            .unwrap();
        assert_eq!(def.health, 2); // no damage taken
        assert!(!def.keywords.contains(&Keyword::DivineShield)); // shield consumed

        assert!(events
            .iter()
            .any(|e| matches!(e, Event::DivineShieldPopped { .. })));
    }

    #[test]
    fn shield_removed_second_hit_deals_damage() {
        let reg = test_registry();
        let deck_p0 = make_deck("test_minion_2_3", 30);
        let deck_p1 = make_deck("test_shield_2_2", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck_p0, &deck_p1, fixed_rng());

        engine.state.players[0].mana = 10;
        // Play two attackers
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();

        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine.state.players[1].mana = 10;
        engine
            .process_action(Action::PlayCard {
                player: 1,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let defender_id = engine.state().players[1].board[0];

        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        let a1 = engine.state().players[0].board[0];
        let a2 = engine.state().players[0].board[1];

        // First hit: pops shield
        engine
            .process_action(Action::Attack {
                player: 0,
                attacker: a1,
                defender: defender_id,
            })
            .unwrap();

        // Second hit: deals damage, kills the 2/2
        engine
            .process_action(Action::Attack {
                player: 0,
                attacker: a2,
                defender: defender_id,
            })
            .unwrap();

        assert!(!engine.state().players[1].board.contains(&defender_id));
        assert!(engine.state().players[1].graveyard.contains(&defender_id));
    }

    #[test]
    fn multiple_taunts_can_choose_either() {
        let reg = test_registry();
        let deck_p0 = make_deck("test_minion_2_3", 30);
        let deck_p1 = make_deck("test_taunt_1_5", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck_p0, &deck_p1, fixed_rng());

        engine.state.players[0].mana = 10;
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let attacker_id = engine.state().players[0].board[0];

        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine.state.players[1].mana = 10;

        // Play two taunts
        engine
            .process_action(Action::PlayCard {
                player: 1,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        engine
            .process_action(Action::PlayCard {
                player: 1,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let taunt1 = engine.state().players[1].board[0];
        let _taunt2 = engine.state().players[1].board[1];

        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        // Can attack either taunt
        let result = engine.process_action(Action::Attack {
            player: 0,
            attacker: attacker_id,
            defender: taunt1,
        });
        assert!(result.is_ok());
    }

    // ==================== Integration Test ====================

    #[test]
    fn full_mini_game_integration_test() {
        let reg = test_registry();
        let deck_p0 = make_deck("test_charge_4_2", 30);
        let deck_p1 = make_deck("test_minion_2_3", 30);
        let (mut engine, events) = GameEngine::new_game(reg, &deck_p0, &deck_p1, fixed_rng());

        assert!(events.iter().any(|e| matches!(e, Event::GameStarted)));
        assert!(!engine.state().game_over);

        // ---- Turn 1 (P0): not enough mana for 4-cost charger, end turn ----
        assert_eq!(engine.state().players[0].mana, 1);
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();

        // ---- Turn 2 (P1): not enough mana, end turn ----
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        // ---- Turn 3 (P0): not enough mana (3), end turn ----
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();

        // ---- Turn 4 (P1): play 2/3 minion (costs 2, has 2 mana) ----
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        // ---- Turn 5 (P0, 3 mana crystals): not enough, end ----
        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();

        // ---- Turn 6 (P1, 3 mana): play a 2/3 ----
        engine.state.players[1].mana = 10;
        engine
            .process_action(Action::PlayCard {
                player: 1,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();

        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        // ---- Turn 7 (P0, 4 mana): play charger (4/2 charge), attack hero ----
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let charger = engine.state().players[0].board[0];

        // Charge: can attack immediately
        engine
            .process_action(Action::Attack {
                player: 0,
                attacker: charger,
                defender: GameEngine::HERO_ENTITY_ID,
            })
            .unwrap();

        assert_eq!(engine.state().players[1].hero.hp, STARTING_HP - 4);

        // Keep attacking with chargers until hero dies
        engine.state.players[1].hero.hp = 4; // shortcut

        engine
            .process_action(Action::EndTurn { player: 0 })
            .unwrap();
        engine
            .process_action(Action::EndTurn { player: 1 })
            .unwrap();

        // Play another charger
        engine
            .process_action(Action::PlayCard {
                player: 0,
                hand_index: 0,
                position: 0,
                target: None,
            })
            .unwrap();
        let charger2 = engine.state().players[0].board.last().copied().unwrap();

        let events = engine
            .process_action(Action::Attack {
                player: 0,
                attacker: charger2,
                defender: GameEngine::HERO_ENTITY_ID,
            })
            .unwrap();

        assert!(engine.state().game_over);
        assert_eq!(engine.state().winner, Some(0));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::GameOver { winner: Some(0) })));
    }

    // ==================== Phase 5 Tests ====================

    #[test]
    fn fireball_deals_damage_to_minion() {
        let reg = test_registry();
        let deck_p0 = make_deck("test_fireball", 30);
        let deck_p1 = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck_p0, &deck_p1, fixed_rng());

        // P1 plays a minion
        engine.process_action(Action::EndTurn { player: 0 }).unwrap();
        engine.state.players[1].mana = 10;
        engine.process_action(Action::PlayCard {
            player: 1,
            hand_index: 0,
            position: 0,
            target: None,
        }).unwrap();
        let target_id = engine.state().players[1].board[0];
        engine.process_action(Action::EndTurn { player: 1 }).unwrap();

        // P0 casts fireball on the minion
        engine.state.players[0].mana = 10;
        engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 0,
            position: 0,
            target: Some(target_id),
        }).unwrap();

        // Minion (2/3) takes 6 damage → dies
        assert!(!engine.state().players[1].board.contains(&target_id));
        assert!(engine.state().players[1].graveyard.contains(&target_id));
    }

    #[test]
    fn fireball_can_target_hero() {
        let reg = test_registry();
        let deck_p0 = make_deck("test_fireball", 30);
        let deck_p1 = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck_p0, &deck_p1, fixed_rng());

        engine.state.players[0].mana = 10;

        let hero_target = GameEngine::hero_entity_id(1);
        engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 0,
            position: 0,
            target: Some(hero_target),
        }).unwrap();

        assert_eq!(engine.state().players[1].hero.hp, STARTING_HP - 6);
    }

    #[test]
    fn spell_requiring_target_without_one_errors() {
        let reg = test_registry();
        let deck = make_deck("test_fireball", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;

        // Fireball requires a target — enemy characters exist (hero), so TargetRequired
        let result = engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 0,
            position: 0,
            target: None,
        });
        assert!(matches!(result, Err(GameError::TargetRequired)));
    }

    #[test]
    fn invalid_target_rejected() {
        let reg = test_registry();
        let deck = make_deck("test_fireball", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;

        // Try to target own hero with EnemyCharacter filter → invalid
        let own_hero = GameEngine::hero_entity_id(0);
        let result = engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 0,
            position: 0,
            target: Some(own_hero),
        });
        assert!(matches!(result, Err(GameError::InvalidTarget(_))));
    }

    #[test]
    fn battlecry_triggers_on_play() {
        let reg = test_registry();
        let deck = make_deck("test_leeroy", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;
        engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 0,
            position: 0,
            target: None,
        }).unwrap();

        // Leeroy summons 2 whelps for opponent (player 1)
        assert_eq!(engine.state().players[1].board.len(), 2);
        // Leeroy is on player 0's board
        assert_eq!(engine.state().players[0].board.len(), 1);
    }

    #[test]
    fn battlecry_does_not_trigger_on_summon_by_effect() {
        let reg = test_registry();
        let deck = make_deck("test_leeroy", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;
        engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 0,
            position: 0,
            target: None,
        }).unwrap();

        // The whelps summoned by Leeroy's battlecry should NOT trigger their own battlecries
        // (tokens don't have battlecry anyway, but verify no extra summons happened)
        assert_eq!(engine.state().players[1].board.len(), 2); // just the 2 whelps
    }

    #[test]
    fn deathrattle_triggers_on_death() {
        let reg = test_registry();
        let deck_p0 = make_deck("test_minion_2_3", 30);
        let deck_p1 = make_deck("test_deathrattle_minion", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck_p0, &deck_p1, fixed_rng());

        // P1 plays deathrattle minion (2/1, deathrattle: draw a card)
        engine.process_action(Action::EndTurn { player: 0 }).unwrap();
        engine.state.players[1].mana = 10;
        engine.process_action(Action::PlayCard {
            player: 1,
            hand_index: 0,
            position: 0,
            target: None,
        }).unwrap();
        let dr_minion = engine.state().players[1].board[0];

        // P0 plays minion and attacks it next turn
        engine.process_action(Action::EndTurn { player: 1 }).unwrap();
        engine.state.players[0].mana = 10;
        engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 0,
            position: 0,
            target: None,
        }).unwrap();

        // Wait a turn for summoning sickness
        engine.process_action(Action::EndTurn { player: 0 }).unwrap();
        engine.process_action(Action::EndTurn { player: 1 }).unwrap();

        let attacker = engine.state().players[0].board[0];
        let p1_hand_before_attack = engine.state().players[1].hand.len();

        // 2/3 attacks 2/1 — deathrattle minion dies, should draw a card for P1
        engine.process_action(Action::Attack {
            player: 0,
            attacker,
            defender: dr_minion,
        }).unwrap();

        assert!(!engine.state().players[1].board.contains(&dr_minion));
        // P1 should have drawn 1 card from deathrattle
        assert_eq!(
            engine.state().players[1].hand.len(),
            p1_hand_before_attack + 1
        );
    }

    #[test]
    fn deathrattle_chain() {
        let reg = test_registry();
        let deck_p0 = make_deck("test_destroy_spell", 30);
        let deck_p1 = make_deck("test_deathrattle_minion", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck_p0, &deck_p1, fixed_rng());

        // P1 plays two deathrattle minions (2/1, deathrattle: draw a card)
        engine.process_action(Action::EndTurn { player: 0 }).unwrap();
        engine.state.players[1].mana = 10;
        engine.process_action(Action::PlayCard {
            player: 1, hand_index: 0, position: 0, target: None,
        }).unwrap();
        engine.process_action(Action::PlayCard {
            player: 1, hand_index: 0, position: 0, target: None,
        }).unwrap();
        let dr1 = engine.state().players[1].board[0];

        engine.process_action(Action::EndTurn { player: 1 }).unwrap();
        engine.state.players[0].mana = 10;

        let p1_hand = engine.state().players[1].hand.len();

        // Destroy first deathrattle minion
        engine.process_action(Action::PlayCard {
            player: 0, hand_index: 0, position: 0, target: Some(dr1),
        }).unwrap();

        // P1 should have drawn 1 card from deathrattle
        assert_eq!(engine.state().players[1].hand.len(), p1_hand + 1);
        assert!(!engine.state().players[1].board.contains(&dr1));
    }

    #[test]
    fn heal_restores_health() {
        let reg = test_registry();
        let deck = make_deck("test_heal_spell", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        // Damage the hero
        engine.state.players[0].hero.hp = 20;
        engine.state.players[0].mana = 10;

        let own_hero = GameEngine::hero_entity_id(0);
        engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 0,
            position: 0,
            target: Some(own_hero),
        }).unwrap();

        assert_eq!(engine.state().players[0].hero.hp, 28);
    }

    #[test]
    fn heal_does_not_exceed_max_health() {
        let reg = test_registry();
        let deck = make_deck("test_heal_spell", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        // Hero at 29 hp, heal 8 should cap at 30
        engine.state.players[0].hero.hp = 29;
        engine.state.players[0].mana = 10;

        let own_hero = GameEngine::hero_entity_id(0);
        engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 0,
            position: 0,
            target: Some(own_hero),
        }).unwrap();

        assert_eq!(engine.state().players[0].hero.hp, STARTING_HP);
    }

    #[test]
    fn buff_increases_stats() {
        let reg = test_registry();
        let deck_p0 = make_deck("test_buff_spell", 30);
        let (mut engine, _) = GameEngine::new_game(reg.clone(), &deck_p0, &deck_p0, fixed_rng());

        engine.state.players[0].mana = 10;

        // Place a minion manually
        let minion_card = reg.get("test_minion_2_3").unwrap();
        let eid = engine.state.alloc_entity_id();
        let entity = Entity::from_card_def(eid, minion_card, 0);
        engine.state.entities.insert(eid, entity);
        engine.state.players[0].board.push(eid);

        // Cast buff on it (+2/+2)
        engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 0,
            position: 0,
            target: Some(eid),
        }).unwrap();

        let m = engine.state().entities.get(&eid).unwrap().as_minion().unwrap();
        assert_eq!(m.attack, 4); // 2+2
        assert_eq!(m.health, 5); // 3+2
    }

    #[test]
    fn draw_cards_effect() {
        let reg = test_registry();
        let deck = make_deck("test_draw_spell", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        engine.state.players[0].mana = 10;
        let hand_before = engine.state().players[0].hand.len();

        engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 0,
            position: 0,
            target: None,
        }).unwrap();

        // Played 1 card (-1) then drew 2 (+2) = net +1
        assert_eq!(engine.state().players[0].hand.len(), hand_before + 1);
    }

    #[test]
    fn destroy_minion_effect() {
        let reg = test_registry();
        let deck_p0 = make_deck("test_destroy_spell", 30);
        let deck_p1 = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck_p0, &deck_p1, fixed_rng());

        // P1 plays a minion
        engine.process_action(Action::EndTurn { player: 0 }).unwrap();
        engine.state.players[1].mana = 10;
        engine.process_action(Action::PlayCard {
            player: 1,
            hand_index: 0,
            position: 0,
            target: None,
        }).unwrap();
        let target = engine.state().players[1].board[0];

        engine.process_action(Action::EndTurn { player: 1 }).unwrap();
        engine.state.players[0].mana = 10;

        engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 0,
            position: 0,
            target: Some(target),
        }).unwrap();

        assert!(!engine.state().players[1].board.contains(&target));
        assert!(engine.state().players[1].graveyard.contains(&target));
    }

    #[test]
    fn summon_respects_board_limit() {
        let reg = test_registry();
        let deck = make_deck("test_leeroy", 30);
        let (mut engine, _) = GameEngine::new_game(reg.clone(), &deck, &deck, fixed_rng());

        // Fill opponent board with 6 minions
        for _ in 0..6 {
            let whelp = reg.get("test_token_whelp").unwrap();
            let eid = engine.state.alloc_entity_id();
            let entity = Entity::from_card_def(eid, whelp, 1);
            engine.state.entities.insert(eid, entity);
            engine.state.players[1].board.push(eid);
        }
        assert_eq!(engine.state().players[1].board.len(), 6);

        // Play Leeroy — tries to summon 2 whelps but only space for 1
        engine.state.players[0].mana = 10;
        engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 0,
            position: 0,
            target: None,
        }).unwrap();

        assert_eq!(engine.state().players[1].board.len(), MAX_BOARD_SIZE); // 7, not 8
    }

    #[test]
    fn effect_depth_limit() {
        // This is a safety test — effects can't recurse infinitely
        // In normal gameplay this is hard to trigger, but the limit exists
        // We verify it doesn't panic by running a deeply nested effect scenario
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        // Manually call execute_effects at depth 19 — should still work
        let effects = vec![crate::effect::Effect::DrawCards { count: 1 }];
        let events = engine.execute_effects(&effects, 0, None, 19);
        assert!(!events.is_empty()); // should draw

        // At depth 20 — should be a no-op
        let events = engine.execute_effects(&effects, 0, None, 20);
        assert!(events.is_empty());
    }

    // --- Hero entity_id sentinel tests ---

    #[test]
    fn hero_sentinel_values() {
        assert_eq!(GameEngine::hero_entity_id(0), 0);
        assert_eq!(GameEngine::hero_entity_id(1), u64::MAX);
    }

    #[test]
    fn hero_sentinels_no_collision_with_allocated_ids() {
        // Normal entity_ids start at 1 and increment; they must never
        // collide with the hero sentinels (0 and u64::MAX).
        let reg = test_registry();
        let deck = make_deck("test_minion_2_3", 30);
        let (engine, _) = GameEngine::new_game(reg, &deck, &deck, fixed_rng());

        let next = engine.state().next_entity_id;
        assert!(next >= 1, "allocated ids start at 1");
        // All allocated ids are in [1, next), none equal to sentinels
        for id in 1..next {
            assert_ne!(id, GameEngine::hero_entity_id(0));
            assert_ne!(id, GameEngine::hero_entity_id(1));
        }
    }

    #[test]
    fn hero_p1_sentinel_wraps_to_negative_i64() {
        // The gdext bridge converts EntityId (u64) to i64 for GDScript.
        // Player 1's hero sentinel u64::MAX becomes -1 as i64.
        // This test documents that invariant so bridge code (NO_TARGET = -2)
        // stays correct.
        let p1_hero = GameEngine::hero_entity_id(1);
        let as_i64 = p1_hero as i64;
        assert_eq!(as_i64, -1, "u64::MAX must wrap to -1 as i64");

        // Roundtrip: i64 → u64 must recover the original sentinel
        let roundtrip = as_i64 as u64;
        assert_eq!(roundtrip, p1_hero);

        // The bridge NO_TARGET sentinel (-2) must NOT collide with either hero
        const NO_TARGET: i64 = -2;
        let p0_hero_i64 = GameEngine::hero_entity_id(0) as i64;
        assert_ne!(NO_TARGET, p0_hero_i64);
        assert_ne!(NO_TARGET, as_i64);
    }

    #[test]
    fn fireball_p1_hero_via_i64_roundtrip() {
        // Simulate the bridge conversion path: hero_entity_id → i64 → u64 → target
        let reg = test_registry();
        let deck_p0 = make_deck("test_fireball", 30);
        let deck_p1 = make_deck("test_minion_2_3", 30);
        let (mut engine, _) = GameEngine::new_game(reg, &deck_p0, &deck_p1, fixed_rng());
        engine.state.players[0].mana = 10;

        // Convert through i64 as the bridge does
        let hero_u64 = GameEngine::hero_entity_id(1);
        let hero_i64 = hero_u64 as i64; // bridge: entity_id_to_i64
        assert_eq!(hero_i64, -1);
        let target = hero_i64 as EntityId; // bridge: i64_to_entity_id

        engine.process_action(Action::PlayCard {
            player: 0,
            hand_index: 0,
            position: 0,
            target: Some(target),
        }).unwrap();

        assert_eq!(engine.state().players[1].hero.hp, STARTING_HP - 6);
    }
}
