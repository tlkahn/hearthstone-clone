use crate::card::CardId;
use crate::types::{EntityId, PlayerId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    GameStarted,
    TurnStarted {
        player: PlayerId,
        turn_number: u32,
    },
    ManaGained {
        player: PlayerId,
        new_crystals: u32,
    },
    ManaRefilled {
        player: PlayerId,
        amount: u32,
    },
    CardDrawn {
        player: PlayerId,
        entity_id: EntityId,
        card_id: CardId,
    },
    CardBurned {
        player: PlayerId,
        entity_id: EntityId,
        card_id: CardId,
    },
    FatigueDamage {
        player: PlayerId,
        damage: u32,
    },
    CardPlayed {
        player: PlayerId,
        entity_id: EntityId,
        card_id: CardId,
        hand_index: usize,
    },
    ManaSpent {
        player: PlayerId,
        amount: u32,
        remaining: u32,
    },
    MinionSummoned {
        player: PlayerId,
        entity_id: EntityId,
        position: usize,
    },
    WeaponEquipped {
        player: PlayerId,
        entity_id: EntityId,
    },
    WeaponDestroyed {
        player: PlayerId,
        entity_id: EntityId,
    },
    SpellCast {
        player: PlayerId,
        entity_id: EntityId,
    },
    AttackPerformed {
        attacker: EntityId,
        defender: EntityId,
    },
    DamageDealt {
        target: EntityId,
        amount: u32,
        source: Option<EntityId>,
    },
    HeroDamaged {
        player: PlayerId,
        amount: u32,
        new_hp: i32,
    },
    DivineShieldPopped {
        entity_id: EntityId,
    },
    MinionDied {
        entity_id: EntityId,
        owner: PlayerId,
    },
    HeroDied {
        player: PlayerId,
    },
    GameOver {
        winner: Option<PlayerId>,
    },
    TurnEnded {
        player: PlayerId,
    },
}
