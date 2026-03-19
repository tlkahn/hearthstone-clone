use serde::{Deserialize, Serialize};

use crate::effect::Effect;

pub type CardId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Rarity {
    Free,
    Common,
    Rare,
    Epic,
    Legendary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Keyword {
    Battlecry,
    Deathrattle,
    Taunt,
    Charge,
    DivineShield,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MinionStats {
    pub attack: u32,
    pub health: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WeaponStats {
    pub attack: u32,
    pub durability: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CardTypeData {
    Minion(MinionStats),
    Spell,
    Weapon(WeaponStats),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardDef {
    pub id: CardId,
    pub name: String,
    pub mana_cost: u32,
    pub card_type: CardTypeData,
    pub rarity: Rarity,
    pub keywords: Vec<Keyword>,
    pub text: String,
    pub art: String,
    #[serde(default)]
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardSet {
    pub name: String,
    pub cards: Vec<CardDef>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn minion_card_deserializes_from_ron() {
        let ron_str = r#"
            CardDef(
                id: "basic_ogre",
                name: "Boulderfist Ogre",
                mana_cost: 6,
                card_type: Minion(MinionStats(attack: 6, health: 7)),
                rarity: Free,
                keywords: [],
                text: "",
                art: "ogre.png",
                effects: [],
            )
        "#;
        let card: super::CardDef = ron::from_str(ron_str).unwrap();
        assert_eq!(card.id, "basic_ogre");
        assert_eq!(card.name, "Boulderfist Ogre");
        assert_eq!(card.mana_cost, 6);
        assert_eq!(card.rarity, super::Rarity::Free);
        assert!(card.keywords.is_empty());
        assert_eq!(card.art, "ogre.png");
        match &card.card_type {
            super::CardTypeData::Minion(stats) => {
                assert_eq!(stats.attack, 6);
                assert_eq!(stats.health, 7);
            }
            _ => panic!("expected Minion"),
        }
    }

    #[test]
    fn spell_card_deserializes_from_ron() {
        let ron_str = r#"
            CardDef(
                id: "basic_fireball",
                name: "Fireball",
                mana_cost: 4,
                card_type: Spell,
                rarity: Free,
                keywords: [],
                text: "Deal 6 damage.",
                art: "fireball.png",
                effects: [DealDamage(amount: 6, target: PlayerChoice(EnemyCharacter))],
            )
        "#;
        let card: super::CardDef = ron::from_str(ron_str).unwrap();
        assert_eq!(card.id, "basic_fireball");
        assert_eq!(card.text, "Deal 6 damage.");
        assert!(matches!(card.card_type, super::CardTypeData::Spell));
        assert_eq!(card.effects.len(), 1);
        assert!(matches!(
            card.effects[0],
            crate::effect::Effect::DealDamage { amount: 6, .. }
        ));
    }

    #[test]
    fn weapon_card_deserializes_from_ron() {
        let ron_str = r#"
            CardDef(
                id: "basic_fiery_war_axe",
                name: "Fiery War Axe",
                mana_cost: 3,
                card_type: Weapon(WeaponStats(attack: 3, durability: 2)),
                rarity: Free,
                keywords: [],
                text: "",
                art: "fiery_war_axe.png",
            )
        "#;
        let card: super::CardDef = ron::from_str(ron_str).unwrap();
        match &card.card_type {
            super::CardTypeData::Weapon(stats) => {
                assert_eq!(stats.attack, 3);
                assert_eq!(stats.durability, 2);
            }
            _ => panic!("expected Weapon"),
        }
        // effects should default to empty when omitted
        assert!(card.effects.is_empty());
    }

    #[test]
    fn card_with_keywords_deserializes() {
        let ron_str = r#"
            CardDef(
                id: "basic_senjin",
                name: "Sen'jin Shieldmasta",
                mana_cost: 4,
                card_type: Minion(MinionStats(attack: 3, health: 5)),
                rarity: Free,
                keywords: [Taunt],
                text: "Taunt",
                art: "senjin.png",
            )
        "#;
        let card: super::CardDef = ron::from_str(ron_str).unwrap();
        assert_eq!(card.keywords, vec![super::Keyword::Taunt]);
    }

    #[test]
    fn card_with_multiple_keywords() {
        let ron_str = r#"
            CardDef(
                id: "basic_leeroy",
                name: "Leeroy Jenkins",
                mana_cost: 5,
                card_type: Minion(MinionStats(attack: 6, health: 2)),
                rarity: Legendary,
                keywords: [Charge, Battlecry],
                text: "Charge. Battlecry: Summon two 1/1 Whelps for your opponent.",
                art: "leeroy.png",
                effects: [Summon(card_id: "token_whelp", count: 2, for_opponent: true)],
            )
        "#;
        let card: super::CardDef = ron::from_str(ron_str).unwrap();
        assert_eq!(card.rarity, super::Rarity::Legendary);
        assert_eq!(card.keywords, vec![super::Keyword::Charge, super::Keyword::Battlecry]);
        assert_eq!(card.effects.len(), 1);
    }

    #[test]
    fn card_set_deserializes_from_ron() {
        let ron_str = r#"
            CardSet(
                name: "Basic",
                cards: [
                    CardDef(
                        id: "a",
                        name: "Card A",
                        mana_cost: 1,
                        card_type: Spell,
                        rarity: Common,
                        keywords: [],
                        text: "",
                        art: "a.png",
                    ),
                    CardDef(
                        id: "b",
                        name: "Card B",
                        mana_cost: 2,
                        card_type: Spell,
                        rarity: Rare,
                        keywords: [],
                        text: "",
                        art: "b.png",
                    ),
                ],
            )
        "#;
        let set: super::CardSet = ron::from_str(ron_str).unwrap();
        assert_eq!(set.name, "Basic");
        assert_eq!(set.cards.len(), 2);
        assert_eq!(set.cards[0].id, "a");
        assert_eq!(set.cards[1].id, "b");
    }
}
