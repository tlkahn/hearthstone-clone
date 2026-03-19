use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

use crate::card::{CardDef, CardId, CardSet};

#[derive(Error, Debug)]
pub enum CardLoadError {
    #[error("IO error reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Parse error in {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: ron::error::SpannedError,
    },
    #[error("Duplicate card id: {0}")]
    DuplicateId(CardId),
    #[error("Validation error for card {id}: {message}")]
    Validation { id: CardId, message: String },
}

#[derive(Debug, Clone)]
pub struct CardRegistry {
    cards: HashMap<CardId, CardDef>,
}

impl CardRegistry {
    pub fn load_from_directory(dir: &Path) -> Result<Self, Vec<CardLoadError>> {
        let mut cards = HashMap::new();
        let mut errors = Vec::new();

        let mut entries: Vec<_> = fs::read_dir(dir)
            .map_err(|e| {
                vec![CardLoadError::Io {
                    path: dir.display().to_string(),
                    source: e,
                }]
            })?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .map_or(false, |ext| ext == "ron")
            })
            .collect();
        entries.sort_by_key(|e| e.path());

        for entry in entries {
            let path = entry.path();
            match Self::load_file(&path) {
                Ok(card_set) => {
                    for card in card_set.cards {
                        if let Err(e) = Self::validate(&card) {
                            errors.push(e);
                            continue;
                        }
                        if cards.contains_key(&card.id) {
                            errors.push(CardLoadError::DuplicateId(card.id.clone()));
                            continue;
                        }
                        cards.insert(card.id.clone(), card);
                    }
                }
                Err(e) => errors.push(e),
            }
        }

        if errors.is_empty() {
            Ok(Self { cards })
        } else {
            Err(errors)
        }
    }

    fn load_file(path: &Path) -> Result<CardSet, CardLoadError> {
        let contents = fs::read_to_string(path).map_err(|e| CardLoadError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        ron::from_str(&contents).map_err(|e| CardLoadError::Parse {
            path: path.display().to_string(),
            source: e,
        })
    }

    fn validate(card: &CardDef) -> Result<(), CardLoadError> {
        if card.id.is_empty() {
            return Err(CardLoadError::Validation {
                id: card.id.clone(),
                message: "Card id must not be empty".into(),
            });
        }
        if card.name.is_empty() {
            return Err(CardLoadError::Validation {
                id: card.id.clone(),
                message: "Card name must not be empty".into(),
            });
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&CardDef> {
        self.cards.get(id)
    }

    pub fn count(&self) -> usize {
        self.cards.len()
    }

    pub fn ids(&self) -> impl Iterator<Item = &CardId> {
        self.cards.keys()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn load_empty_directory_produces_empty_registry() {
        let dir = TempDir::new().unwrap();
        let registry = super::CardRegistry::load_from_directory(dir.path()).unwrap();
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn load_single_file_with_two_cards() {
        let dir = TempDir::new().unwrap();
        let ron = r#"
            CardSet(
                name: "Test",
                cards: [
                    CardDef(
                        id: "card_a",
                        name: "Card A",
                        mana_cost: 1,
                        card_type: Spell,
                        rarity: Common,
                        keywords: [],
                        text: "",
                        art: "a.png",
                    ),
                    CardDef(
                        id: "card_b",
                        name: "Card B",
                        mana_cost: 2,
                        card_type: Minion(MinionStats(attack: 2, health: 3)),
                        rarity: Rare,
                        keywords: [Taunt],
                        text: "Taunt",
                        art: "b.png",
                    ),
                ],
            )
        "#;
        fs::write(dir.path().join("test.ron"), ron).unwrap();

        let registry = super::CardRegistry::load_from_directory(dir.path()).unwrap();
        assert_eq!(registry.count(), 2);
        let a = registry.get("card_a").unwrap();
        assert_eq!(a.name, "Card A");
        let b = registry.get("card_b").unwrap();
        assert_eq!(b.name, "Card B");
    }

    #[test]
    fn load_multiple_files_merges_cards() {
        let dir = TempDir::new().unwrap();
        let ron1 = r#"
            CardSet(name: "Set1", cards: [
                CardDef(id: "s1_a", name: "A", mana_cost: 1, card_type: Spell, rarity: Free, keywords: [], text: "", art: "a.png"),
            ])
        "#;
        let ron2 = r#"
            CardSet(name: "Set2", cards: [
                CardDef(id: "s2_b", name: "B", mana_cost: 2, card_type: Spell, rarity: Free, keywords: [], text: "", art: "b.png"),
            ])
        "#;
        fs::write(dir.path().join("set1.ron"), ron1).unwrap();
        fs::write(dir.path().join("set2.ron"), ron2).unwrap();

        let registry = super::CardRegistry::load_from_directory(dir.path()).unwrap();
        assert_eq!(registry.count(), 2);
        assert!(registry.get("s1_a").is_some());
        assert!(registry.get("s2_b").is_some());
    }

    #[test]
    fn duplicate_id_across_files_returns_error() {
        let dir = TempDir::new().unwrap();
        let ron1 = r#"
            CardSet(name: "Set1", cards: [
                CardDef(id: "dupe", name: "A", mana_cost: 1, card_type: Spell, rarity: Free, keywords: [], text: "", art: "a.png"),
            ])
        "#;
        let ron2 = r#"
            CardSet(name: "Set2", cards: [
                CardDef(id: "dupe", name: "B", mana_cost: 2, card_type: Spell, rarity: Free, keywords: [], text: "", art: "b.png"),
            ])
        "#;
        fs::write(dir.path().join("set1.ron"), ron1).unwrap();
        fs::write(dir.path().join("set2.ron"), ron2).unwrap();

        let result = super::CardRegistry::load_from_directory(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn empty_card_id_returns_error() {
        let dir = TempDir::new().unwrap();
        let ron = r#"
            CardSet(name: "Bad", cards: [
                CardDef(id: "", name: "No ID", mana_cost: 1, card_type: Spell, rarity: Free, keywords: [], text: "", art: "x.png"),
            ])
        "#;
        fs::write(dir.path().join("bad.ron"), ron).unwrap();

        let result = super::CardRegistry::load_from_directory(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn empty_card_name_returns_error() {
        let dir = TempDir::new().unwrap();
        let ron = r#"
            CardSet(name: "Bad", cards: [
                CardDef(id: "no_name", name: "", mana_cost: 1, card_type: Spell, rarity: Free, keywords: [], text: "", art: "x.png"),
            ])
        "#;
        fs::write(dir.path().join("bad.ron"), ron).unwrap();

        let result = super::CardRegistry::load_from_directory(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn non_ron_files_are_ignored() {
        let dir = TempDir::new().unwrap();
        let ron = r#"
            CardSet(name: "Good", cards: [
                CardDef(id: "good", name: "Good", mana_cost: 1, card_type: Spell, rarity: Free, keywords: [], text: "", art: "g.png"),
            ])
        "#;
        fs::write(dir.path().join("cards.ron"), ron).unwrap();
        fs::write(dir.path().join("readme.txt"), "ignore me").unwrap();

        let registry = super::CardRegistry::load_from_directory(dir.path()).unwrap();
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn malformed_ron_returns_error() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("bad.ron"), "not valid ron {{{{").unwrap();

        let result = super::CardRegistry::load_from_directory(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn get_nonexistent_card_returns_none() {
        let dir = TempDir::new().unwrap();
        let registry = super::CardRegistry::load_from_directory(dir.path()).unwrap();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn load_actual_data_files() {
        // Integration test: load the real data/cards/ directory
        let data_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("data")
            .join("cards");
        let registry = super::CardRegistry::load_from_directory(&data_dir).unwrap();

        // We have 6 minions + 1 spell + 1 token = 8 cards
        assert_eq!(registry.count(), 8);

        // Spot-check specific cards
        let ogre = registry.get("basic_boulderfist_ogre").unwrap();
        assert_eq!(ogre.name, "Boulderfist Ogre");
        assert_eq!(ogre.mana_cost, 6);

        let fireball = registry.get("basic_fireball").unwrap();
        assert_eq!(fireball.text, "Deal 6 damage.");
        assert!(matches!(fireball.card_type, crate::card::CardTypeData::Spell));

        let leeroy = registry.get("basic_leeroy_jenkins").unwrap();
        assert_eq!(leeroy.keywords.len(), 2);
    }

    #[test]
    fn ids_returns_all_card_ids() {
        let dir = TempDir::new().unwrap();
        let ron = r#"
            CardSet(name: "Test", cards: [
                CardDef(id: "x", name: "X", mana_cost: 1, card_type: Spell, rarity: Free, keywords: [], text: "", art: "x.png"),
                CardDef(id: "y", name: "Y", mana_cost: 2, card_type: Spell, rarity: Free, keywords: [], text: "", art: "y.png"),
            ])
        "#;
        fs::write(dir.path().join("test.ron"), ron).unwrap();
        let registry = super::CardRegistry::load_from_directory(dir.path()).unwrap();
        let mut ids: Vec<&String> = registry.ids().collect();
        ids.sort();
        assert_eq!(ids, vec!["x", "y"]);
    }
}
