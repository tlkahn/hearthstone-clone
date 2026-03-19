use crate::effect::{Effect, TargetFilter, TargetSpec};
use crate::engine::GameEngine;
use crate::entity::Entity;
use crate::event::Event;
use crate::types::*;

const MAX_EFFECT_DEPTH: u32 = 20;

impl GameEngine {
    pub(crate) fn execute_effects(
        &mut self,
        effects: &[Effect],
        caster: PlayerId,
        target: Option<EntityId>,
        depth: u32,
    ) -> Vec<Event> {
        if depth >= MAX_EFFECT_DEPTH {
            return Vec::new();
        }

        let mut events = Vec::new();
        for effect in effects {
            events.extend(self.execute_single_effect(effect, caster, target, depth));
        }
        events
    }

    fn execute_single_effect(
        &mut self,
        effect: &Effect,
        caster: PlayerId,
        chosen_target: Option<EntityId>,
        depth: u32,
    ) -> Vec<Event> {
        match effect {
            Effect::DealDamage { amount, target } => {
                let targets = self.resolve_targets(target, caster, chosen_target);
                let mut events = Vec::new();
                for tid in targets {
                    events.extend(self.deal_damage_to(tid, *amount, caster, depth));
                }
                events
            }
            Effect::Heal { amount, target } => {
                let targets = self.resolve_targets(target, caster, chosen_target);
                let mut events = Vec::new();
                for tid in targets {
                    events.extend(self.heal_target(tid, *amount, caster));
                }
                events
            }
            Effect::Summon {
                card_id,
                count,
                for_opponent,
            } => {
                let owner = if *for_opponent {
                    self.state.opponent(caster)
                } else {
                    caster
                };
                let mut events = Vec::new();
                for _ in 0..*count {
                    if self.state.players[owner].board.len() >= MAX_BOARD_SIZE {
                        break;
                    }
                    if let Some(card_def) = self.registry().get(card_id).cloned() {
                        let eid = self.state.alloc_entity_id();
                        let entity = Entity::from_card_def(eid, &card_def, owner);
                        self.state.entities.insert(eid, entity);
                        let pos = self.state.players[owner].board.len();
                        self.state.players[owner].board.push(eid);
                        events.push(Event::MinionSummoned {
                            player: owner,
                            entity_id: eid,
                            position: pos,
                        });
                    }
                }
                events
            }
            Effect::DrawCards { count } => {
                let mut events = Vec::new();
                for _ in 0..*count {
                    events.extend(self.draw_card(caster));
                }
                events
            }
            Effect::BuffMinion {
                attack,
                health,
                target,
            } => {
                let targets = self.resolve_targets(target, caster, chosen_target);
                for tid in targets {
                    if let Some(minion) = self
                        .state
                        .entities
                        .get_mut(&tid)
                        .and_then(|e| e.as_minion_mut())
                    {
                        minion.attack = (minion.attack as i32 + attack).max(0) as u32;
                        minion.health += *health as i32;
                        minion.max_health += *health as i32;
                    }
                }
                Vec::new()
            }
            Effect::DestroyMinion { target } => {
                let targets = self.resolve_targets(target, caster, chosen_target);
                let mut events = Vec::new();
                for tid in targets {
                    if let Some(minion) = self
                        .state
                        .entities
                        .get_mut(&tid)
                        .and_then(|e| e.as_minion_mut())
                    {
                        minion.health = 0;
                    }
                }
                events.extend(self.process_deaths_with_deathrattles(depth + 1));
                events
            }
        }
    }

    fn deal_damage_to(
        &mut self,
        target_id: EntityId,
        amount: u32,
        _source_player: PlayerId,
        depth: u32,
    ) -> Vec<Event> {
        let mut events = Vec::new();

        // Check if target is a hero (HERO_ENTITY_ID=0 for opponent, or check both players)
        // We'll check if target_id is on any board first
        let is_minion = self
            .state
            .entities
            .get(&target_id)
            .map_or(false, |e| e.is_minion());

        if is_minion {
            // Check divine shield
            let has_shield = self
                .state
                .entities
                .get(&target_id)
                .and_then(|e| e.as_minion())
                .map_or(false, |m| {
                    m.keywords.contains(&crate::card::Keyword::DivineShield)
                });

            if has_shield {
                if let Some(m) = self
                    .state
                    .entities
                    .get_mut(&target_id)
                    .and_then(|e| e.as_minion_mut())
                {
                    m.keywords.remove(&crate::card::Keyword::DivineShield);
                }
                events.push(Event::DivineShieldPopped {
                    entity_id: target_id,
                });
            } else {
                if let Some(m) = self
                    .state
                    .entities
                    .get_mut(&target_id)
                    .and_then(|e| e.as_minion_mut())
                {
                    m.health -= amount as i32;
                }
                events.push(Event::DamageDealt {
                    target: target_id,
                    amount,
                    source: None,
                });
            }

            events.extend(self.process_deaths_with_deathrattles(depth + 1));
        } else {
            // Target is a hero — find which player
            // Hero target: entity_id resolved at targeting time via hero_entity_id()
            // Find which player this hero belongs to
            let player = self.find_hero_player(target_id);
            if let Some(p) = player {
                self.state.players[p].hero.hp -= amount as i32;
                events.push(Event::DamageDealt {
                    target: target_id,
                    amount,
                    source: None,
                });
                events.push(Event::HeroDamaged {
                    player: p,
                    amount,
                    new_hp: self.state.players[p].hero.hp,
                });
                if self.state.players[p].hero.is_dead() && !self.state.game_over {
                    events.push(Event::HeroDied { player: p });
                    self.state.game_over = true;
                    self.state.winner = Some(self.state.opponent(p));
                    events.push(Event::GameOver {
                        winner: self.state.winner,
                    });
                }
            }
        }

        events
    }

    fn heal_target(
        &mut self,
        target_id: EntityId,
        amount: u32,
        _source_player: PlayerId,
    ) -> Vec<Event> {
        let is_minion = self
            .state
            .entities
            .get(&target_id)
            .map_or(false, |e| e.is_minion());

        if is_minion {
            if let Some(m) = self
                .state
                .entities
                .get_mut(&target_id)
                .and_then(|e| e.as_minion_mut())
            {
                let _old = m.health;
                m.health = (m.health + amount as i32).min(m.max_health);
            }
        } else {
            if let Some(p) = self.find_hero_player(target_id) {
                let _old = self.state.players[p].hero.hp;
                self.state.players[p].hero.hp = (self.state.players[p].hero.hp + amount as i32)
                    .min(self.state.players[p].hero.max_hp);
            }
        }

        Vec::new()
    }

    pub(crate) fn process_deaths_with_deathrattles(&mut self, depth: u32) -> Vec<Event> {
        let mut events = Vec::new();

        for player_id in 0..2 {
            let mut dead = Vec::new();
            self.state.players[player_id].board.retain(|&eid| {
                let alive = self
                    .state
                    .entities
                    .get(&eid)
                    .and_then(|e| e.as_minion())
                    .map_or(true, |m| m.health > 0);
                if !alive {
                    dead.push(eid);
                }
                alive
            });

            for eid in dead {
                self.state.players[player_id].graveyard.push(eid);
                events.push(Event::MinionDied {
                    entity_id: eid,
                    owner: player_id,
                });

                // Execute deathrattle effects
                let card_id = self
                    .state
                    .entities
                    .get(&eid)
                    .map(|e| e.card_id.clone())
                    .unwrap_or_default();
                let has_deathrattle = self
                    .state
                    .entities
                    .get(&eid)
                    .and_then(|e| e.as_minion())
                    .map_or(false, |m| {
                        m.keywords.contains(&crate::card::Keyword::Deathrattle)
                    });

                if has_deathrattle {
                    if let Some(card_def) = self.registry().get(&card_id).cloned() {
                        let dr_effects: Vec<_> = card_def.effects.clone();
                        events.extend(self.execute_effects(&dr_effects, player_id, None, depth));
                    }
                }
            }
        }

        events
    }

    fn resolve_targets(
        &self,
        spec: &TargetSpec,
        caster: PlayerId,
        chosen: Option<EntityId>,
    ) -> Vec<EntityId> {
        match spec {
            TargetSpec::None => Vec::new(),
            TargetSpec::PlayerChoice(_) => {
                chosen.into_iter().collect()
            }
            TargetSpec::Self_ => Vec::new(), // would need entity context
            TargetSpec::All(filter) => self.get_all_matching(filter, caster),
            TargetSpec::Random(filter) => {
                let candidates = self.get_all_matching(filter, caster);
                if candidates.is_empty() {
                    return Vec::new();
                }
                // For determinism, just pick first (RNG would need &mut self)
                vec![candidates[0]]
            }
        }
    }

    fn get_all_matching(&self, filter: &TargetFilter, caster: PlayerId) -> Vec<EntityId> {
        let opp = self.state.opponent(caster);
        match filter {
            TargetFilter::Any => {
                let mut result: Vec<EntityId> = Vec::new();
                result.extend(&self.state.players[caster].board);
                result.extend(&self.state.players[opp].board);
                result
            }
            TargetFilter::AnyMinion => {
                let mut result: Vec<EntityId> = Vec::new();
                result.extend(&self.state.players[caster].board);
                result.extend(&self.state.players[opp].board);
                result
            }
            TargetFilter::FriendlyMinion => self.state.players[caster].board.clone(),
            TargetFilter::EnemyMinion => self.state.players[opp].board.clone(),
            TargetFilter::AnyCharacter => {
                let mut result = Vec::new();
                // Heroes use sentinel IDs
                result.push(Self::hero_entity_id(caster));
                result.push(Self::hero_entity_id(opp));
                result.extend(&self.state.players[caster].board);
                result.extend(&self.state.players[opp].board);
                result
            }
            TargetFilter::EnemyCharacter => {
                let mut result = Vec::new();
                result.push(Self::hero_entity_id(opp));
                result.extend(&self.state.players[opp].board);
                result
            }
            TargetFilter::FriendlyCharacter => {
                let mut result = Vec::new();
                result.push(Self::hero_entity_id(caster));
                result.extend(&self.state.players[caster].board);
                result
            }
        }
    }

    pub fn hero_entity_id(player: PlayerId) -> EntityId {
        // Use sentinel values: 0 for player 0's hero, u64::MAX for player 1's hero
        if player == 0 {
            0
        } else {
            u64::MAX
        }
    }

    fn find_hero_player(&self, entity_id: EntityId) -> Option<PlayerId> {
        if entity_id == Self::hero_entity_id(0) {
            Some(0)
        } else if entity_id == Self::hero_entity_id(1) {
            Some(1)
        } else {
            None
        }
    }

    pub fn valid_targets(
        &self,
        effects: &[Effect],
        caster: PlayerId,
    ) -> Option<Vec<EntityId>> {
        for effect in effects {
            if effect.requires_target() {
                if let Some(spec) = effect.target_spec() {
                    if let TargetSpec::PlayerChoice(filter) = spec {
                        let targets = self.get_all_matching(filter, caster);
                        return Some(targets);
                    }
                }
            }
        }
        None
    }

    pub fn any_effect_requires_target(effects: &[Effect]) -> bool {
        effects.iter().any(|e| e.requires_target())
    }
}
