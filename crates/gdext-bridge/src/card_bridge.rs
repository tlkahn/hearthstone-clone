use godot::builtin::VarDictionary;
use godot::prelude::*;
use hs_rules::card::{CardDef, CardTypeData, Keyword, Rarity};
use hs_rules::card_loader::CardRegistry;
use std::path::PathBuf;

#[derive(GodotClass)]
#[class(base=Node)]
pub struct CardDatabase {
    registry: Option<CardRegistry>,
    base: Base<Node>,
}

#[godot_api]
impl INode for CardDatabase {
    fn init(base: Base<Node>) -> Self {
        Self {
            registry: None,
            base,
        }
    }

    fn ready(&mut self) {
        // Auto-load cards from data/cards/ relative to the Godot project root.
        let project_path = godot::classes::ProjectSettings::singleton()
            .globalize_path("res://..")
            .to_string();
        let data_dir = PathBuf::from(project_path).join("data").join("cards");
        self.load_cards_from_path(data_dir);
    }
}

#[godot_api]
impl CardDatabase {
    #[func]
    pub fn load_cards(&mut self, path: GString) {
        self.load_cards_from_path(PathBuf::from(path.to_string()));
    }

    #[func]
    pub fn reload_cards(&mut self) {
        self.ready();
    }

    #[func]
    pub fn get_card(&self, id: GString) -> VarDictionary {
        let Some(registry) = &self.registry else {
            godot_error!("CardDatabase: registry not loaded");
            return VarDictionary::new();
        };
        let Some(card) = registry.get(&id.to_string()) else {
            godot_error!("CardDatabase: card not found: {}", id);
            return VarDictionary::new();
        };
        Self::card_to_dict(card)
    }

    #[func]
    pub fn get_all_card_ids(&self) -> Array<GString> {
        let mut arr = Array::new();
        if let Some(registry) = &self.registry {
            for id in registry.ids() {
                arr.push(&GString::from(id.as_str()));
            }
        }
        arr
    }

    #[func]
    pub fn get_card_count(&self) -> i64 {
        self.registry.as_ref().map_or(0, |r| r.count() as i64)
    }
}

impl CardDatabase {
    fn load_cards_from_path(&mut self, path: PathBuf) {
        match CardRegistry::load_from_directory(&path) {
            Ok(registry) => {
                godot_print!(
                    "CardDatabase: loaded {} cards from {}",
                    registry.count(),
                    path.display()
                );
                self.registry = Some(registry);
            }
            Err(errors) => {
                for e in &errors {
                    godot_error!("CardDatabase: {}", e);
                }
            }
        }
    }

    fn card_to_dict(card: &CardDef) -> VarDictionary {
        let mut dict = VarDictionary::new();
        dict.set("id", GString::from(card.id.as_str()));
        dict.set("name", GString::from(card.name.as_str()));
        dict.set("mana_cost", card.mana_cost as i64);
        dict.set("text", GString::from(card.text.as_str()));
        dict.set("art", GString::from(card.art.as_str()));
        dict.set("rarity", GString::from(rarity_str(&card.rarity)));

        match &card.card_type {
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
        for kw in &card.keywords {
            kw_arr.push(&GString::from(keyword_str(kw)));
        }
        dict.set("keywords", kw_arr);

        dict
    }
}

fn rarity_str(r: &Rarity) -> &'static str {
    match r {
        Rarity::Free => "free",
        Rarity::Common => "common",
        Rarity::Rare => "rare",
        Rarity::Epic => "epic",
        Rarity::Legendary => "legendary",
    }
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
