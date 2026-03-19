//! Smoke tests for the Game Rules Engine (System 2).
//!
//! These are high-level integration tests that play out recognisable
//! Hearthstone scenarios against the public API.  They're meant to be
//! read top-to-bottom as a tour of how the engine works.

use std::sync::Arc;

use hs_rules::*;
use hs_rules::action::Action;
use hs_rules::event::Event;
use hs_rules::types::*;

use rand::rngs::StdRng;
use rand::SeedableRng;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn load_test_cards() -> Arc<CardRegistry> {
    let dir = tempfile::TempDir::new().unwrap();
    let ron = r#"
        CardSet(name: "Smoke", cards: [
            // --- vanilla minions ---
            CardDef(id: "wisp",       name: "Wisp",              mana_cost: 0, card_type: Minion(MinionStats(attack: 1, health: 1)), rarity: Free, keywords: [],           text: "", art: "t.png"),
            CardDef(id: "yeti",       name: "Chillwind Yeti",    mana_cost: 4, card_type: Minion(MinionStats(attack: 4, health: 5)), rarity: Free, keywords: [],           text: "", art: "t.png"),
            CardDef(id: "ogre",       name: "Boulderfist Ogre",  mana_cost: 6, card_type: Minion(MinionStats(attack: 6, health: 7)), rarity: Free, keywords: [],           text: "", art: "t.png"),

            // --- keyword minions ---
            CardDef(id: "taunt_3_5",  name: "Sen'jin Shieldmasta", mana_cost: 4, card_type: Minion(MinionStats(attack: 3, health: 5)), rarity: Free, keywords: [Taunt],    text: "Taunt",         art: "t.png"),
            CardDef(id: "charge_4_2", name: "Kor'kron Elite",     mana_cost: 4, card_type: Minion(MinionStats(attack: 4, health: 2)), rarity: Free, keywords: [Charge],     text: "Charge",        art: "t.png"),
            CardDef(id: "shield_3_3", name: "Scarlet Crusader",   mana_cost: 3, card_type: Minion(MinionStats(attack: 3, health: 1)), rarity: Free, keywords: [DivineShield], text: "Divine Shield", art: "t.png"),

            // --- spells ---
            CardDef(id: "fireball",   name: "Fireball",  mana_cost: 4, card_type: Spell, rarity: Free, keywords: [], text: "Deal 6 damage.",  art: "t.png", effects: [DealDamage(amount: 6, target: PlayerChoice(EnemyCharacter))]),
            CardDef(id: "heal8",      name: "Heal",      mana_cost: 3, card_type: Spell, rarity: Free, keywords: [], text: "Restore 8 HP.",   art: "t.png", effects: [Heal(amount: 8, target: PlayerChoice(AnyCharacter))]),
            CardDef(id: "draw2",      name: "Intellect", mana_cost: 3, card_type: Spell, rarity: Free, keywords: [], text: "Draw 2 cards.",    art: "t.png", effects: [DrawCards(count: 2)]),

            // --- battlecry / deathrattle ---
            CardDef(id: "leeroy",     name: "Leeroy Jenkins", mana_cost: 5, card_type: Minion(MinionStats(attack: 6, health: 2)), rarity: Legendary, keywords: [Charge, Battlecry], text: "Charge. Battlecry: Summon two 1/1 Whelps for your opponent.", art: "t.png", effects: [Summon(card_id: "whelp", count: 2, for_opponent: true)]),
            CardDef(id: "whelp",      name: "Whelp",          mana_cost: 1, card_type: Minion(MinionStats(attack: 1, health: 1)), rarity: Free, keywords: [], text: "", art: "t.png"),
            CardDef(id: "loot",       name: "Loot Hoarder",   mana_cost: 2, card_type: Minion(MinionStats(attack: 2, health: 1)), rarity: Free, keywords: [Deathrattle], text: "Deathrattle: Draw a card.", art: "t.png", effects: [DrawCards(count: 1)]),
        ])
    "#;
    std::fs::write(dir.path().join("smoke.ron"), ron).unwrap();
    // Keep the TempDir alive for the duration of the Arc
    let reg = CardRegistry::load_from_directory(dir.path()).unwrap();
    Arc::new(reg)
}

fn deck_of(id: &str, n: usize) -> Vec<CardId> {
    vec![id.to_string(); n]
}

fn rng() -> Box<StdRng> {
    Box::new(StdRng::seed_from_u64(1337))
}

/// Shorthand: skip turns until the active player has `target_mana` crystals.
fn advance_to_mana(engine: &mut GameEngine, target_mana: u32) {
    while engine.state().players[engine.state().active_player].mana_crystals < target_mana {
        let p = engine.state().active_player;
        engine.process_action(Action::EndTurn { player: p }).unwrap();
    }
}

/// Pretty-print an event list (for `cargo test -- --nocapture`).
fn print_events(label: &str, events: &[Event]) {
    println!("--- {label} ---");
    for (i, ev) in events.iter().enumerate() {
        println!("  [{i}] {ev:?}");
    }
    println!();
}

// ---------------------------------------------------------------------------
// Smoke tests
// ---------------------------------------------------------------------------

/// A complete 0-cost Wisp game: play free minions, attack, someone dies.
#[test]
fn wisps_to_the_death() {
    let reg = load_test_cards();
    let (mut engine, init) = GameEngine::new_game(reg, &deck_of("wisp", 30), &deck_of("wisp", 30), rng());
    print_events("game start", &init);

    // Both players spam free wisps every turn, then attack with whatever
    // survived from last turn.  First hero to 0 loses.
    let mut turn = 0;
    while !engine.state().game_over {
        turn += 1;
        let p = engine.state().active_player;

        // Play all wisps we can (0 mana, so limited only by hand/board)
        while !engine.state().players[p].hand.is_empty()
            && engine.state().players[p].board.len() < MAX_BOARD_SIZE
        {
            engine
                .process_action(Action::PlayCard {
                    player: p,
                    hand_index: 0,
                    position: engine.state().players[p].board.len(),
                    target: None,
                })
                .unwrap();
        }

        // Attack with every non-sick minion
        let opp = 1 - p;
        let attackers: Vec<EntityId> = engine.state().players[p].board.clone();
        for atk_id in attackers {
            if engine.state().game_over {
                break;
            }
            let minion = match engine.state().entities.get(&atk_id).and_then(|e| e.as_minion()) {
                Some(m) if !m.summoning_sickness && m.attacks_this_turn == 0 => true,
                _ => false,
            };
            if !minion {
                continue;
            }

            // Pick first enemy minion, or face
            let target = engine.state().players[opp]
                .board
                .first()
                .copied()
                .unwrap_or(GameEngine::HERO_ENTITY_ID);

            let _ = engine.process_action(Action::Attack {
                player: p,
                attacker: atk_id,
                defender: target,
            });
        }

        if !engine.state().game_over {
            engine.process_action(Action::EndTurn { player: p }).unwrap();
        }

        assert!(turn < 200, "game should end before turn 200");
    }

    let winner = engine.state().winner.expect("should have a winner");
    let loser = 1 - winner;
    println!("Game ended on turn {turn}. Player {winner} wins!");
    println!(
        "  Winner HP: {}  |  Loser HP: {}",
        engine.state().players[winner].hero.hp,
        engine.state().players[loser].hero.hp,
    );
    assert!(engine.state().players[loser].hero.hp <= 0);
}

/// Play a Chillwind Yeti, wait a turn, smack the opponent's face a few times.
#[test]
fn yeti_beatdown() {
    let reg = load_test_cards();
    let (mut engine, _) = GameEngine::new_game(reg, &deck_of("yeti", 30), &deck_of("yeti", 30), rng());

    // Skip to turn with 4 mana for both players
    advance_to_mana(&mut engine, 4);
    let p = engine.state().active_player;
    assert!(p == 0 || p == 1); // sanity

    // Play a Yeti (4 mana, 4/5)
    let events = engine
        .process_action(Action::PlayCard {
            player: p,
            hand_index: 0,
            position: 0,
            target: None,
        })
        .unwrap();
    print_events("play yeti", &events);

    let yeti_id = engine.state().players[p].board[0];
    println!("Yeti entity id: {yeti_id}");

    // End turn twice so summoning sickness clears
    engine.process_action(Action::EndTurn { player: p }).unwrap();
    engine.process_action(Action::EndTurn { player: 1 - p }).unwrap();

    // Swing at face — 4 damage
    let opp = 1 - p;
    let events = engine
        .process_action(Action::Attack {
            player: p,
            attacker: yeti_id,
            defender: GameEngine::HERO_ENTITY_ID,
        })
        .unwrap();
    print_events("yeti attacks hero", &events);
    assert_eq!(engine.state().players[opp].hero.hp, STARTING_HP - 4);
    println!("Opponent HP after Yeti swing: {}", engine.state().players[opp].hero.hp);
}

/// Fireball the opponent's face, verify damage and event stream.
#[test]
fn fireball_to_face() {
    let reg = load_test_cards();
    let (mut engine, _) = GameEngine::new_game(reg, &deck_of("fireball", 30), &deck_of("yeti", 30), rng());

    advance_to_mana(&mut engine, 4);
    let p = engine.state().active_player;

    let opp_hero = GameEngine::hero_entity_id(1 - p);
    let events = engine
        .process_action(Action::PlayCard {
            player: p,
            hand_index: 0,
            position: 0,
            target: Some(opp_hero),
        })
        .unwrap();
    print_events("fireball to face", &events);

    let opp = 1 - p;
    assert_eq!(engine.state().players[opp].hero.hp, STARTING_HP - 6);
    println!("Opponent HP: {} (took 6 from Fireball)", engine.state().players[opp].hero.hp);

    // Verify the event stream contains the expected sequence
    assert!(events.iter().any(|e| matches!(e, Event::SpellCast { .. })));
    assert!(events.iter().any(|e| matches!(e, Event::DamageDealt { amount: 6, .. })));
    assert!(events.iter().any(|e| matches!(e, Event::HeroDamaged { amount: 6, .. })));
}

/// Taunt blocks face attacks; killing the taunt lets us through.
#[test]
fn taunt_wall_and_breakthrough() {
    let reg = load_test_cards();
    let (mut engine, _) = GameEngine::new_game(
        reg.clone(),
        &deck_of("charge_4_2", 30),
        &deck_of("taunt_3_5", 30),
        rng(),
    );

    advance_to_mana(&mut engine, 4);
    let p = engine.state().active_player;
    let opp = 1 - p;

    // P plays Kor'kron Elite (4/2 Charge)
    engine.process_action(Action::PlayCard {
        player: p, hand_index: 0, position: 0, target: None,
    }).unwrap();
    let charger = engine.state().players[p].board[0];

    // Opponent plays Sen'jin (3/5 Taunt)
    engine.process_action(Action::EndTurn { player: p }).unwrap();
    engine.state.players[opp].mana = 10;
    engine.process_action(Action::PlayCard {
        player: opp, hand_index: 0, position: 0, target: None,
    }).unwrap();
    let taunt = engine.state().players[opp].board[0];
    engine.process_action(Action::EndTurn { player: opp }).unwrap();

    // Try face — blocked
    let err = engine.process_action(Action::Attack {
        player: p, attacker: charger, defender: GameEngine::HERO_ENTITY_ID,
    }).unwrap_err();
    println!("Face blocked: {err}");
    assert!(matches!(err, GameError::MustAttackTaunt));

    // Hit the taunt instead (4 into 5 → taunt survives at 3/1, charger dies from 3 counter-dmg)
    let events = engine.process_action(Action::Attack {
        player: p, attacker: charger, defender: taunt,
    }).unwrap();
    print_events("charger into taunt", &events);

    // Charger (2 HP) took 3 from the taunt → dead
    assert!(engine.state().players[p].graveyard.iter().any(|&e| e == charger));
    // Taunt survived at 1 HP
    let taunt_hp = engine.state().entities.get(&taunt).unwrap().as_minion().unwrap().health;
    println!("Taunt HP after trade: {taunt_hp}");
    assert_eq!(taunt_hp, 1);

    // Play a second charger and kill the taunt
    engine.state.players[p].mana = 10;
    engine.process_action(Action::PlayCard {
        player: p, hand_index: 0, position: 0, target: None,
    }).unwrap();
    let charger2 = engine.state().players[p].board[0];

    engine.process_action(Action::Attack {
        player: p, attacker: charger2, defender: taunt,
    }).unwrap();
    assert!(engine.state().players[opp].board.is_empty(), "taunt is dead");

    // Now a third charger can go face
    engine.process_action(Action::EndTurn { player: p }).unwrap();
    engine.process_action(Action::EndTurn { player: opp }).unwrap();
    engine.state.players[p].mana = 10;
    engine.process_action(Action::PlayCard {
        player: p, hand_index: 0, position: 0, target: None,
    }).unwrap();
    let charger3 = *engine.state().players[p].board.last().unwrap();

    let events = engine.process_action(Action::Attack {
        player: p, attacker: charger3, defender: GameEngine::HERO_ENTITY_ID,
    }).unwrap();
    print_events("charger goes face", &events);
    println!("Opponent HP: {}", engine.state().players[opp].hero.hp);
    assert_eq!(engine.state().players[opp].hero.hp, STARTING_HP - 4);
}

/// Divine Shield absorbs one hit, then the minion takes real damage.
#[test]
fn divine_shield_pops_then_real_damage() {
    let reg = load_test_cards();
    let (mut engine, _) = GameEngine::new_game(
        reg,
        &deck_of("yeti", 30),
        &deck_of("shield_3_3", 30),
        rng(),
    );

    // Both get enough mana
    advance_to_mana(&mut engine, 4);
    let p = engine.state().active_player;
    let opp = 1 - p;

    // P plays Yeti (4/5)
    engine.process_action(Action::PlayCard {
        player: p, hand_index: 0, position: 0, target: None,
    }).unwrap();
    let yeti = engine.state().players[p].board[0];

    engine.process_action(Action::EndTurn { player: p }).unwrap();
    engine.state.players[opp].mana = 10;

    // Opp plays Scarlet Crusader (3/1 Divine Shield)
    engine.process_action(Action::PlayCard {
        player: opp, hand_index: 0, position: 0, target: None,
    }).unwrap();
    let crusader = engine.state().players[opp].board[0];

    engine.process_action(Action::EndTurn { player: opp }).unwrap();

    // Yeti (4/5) attacks Crusader (3/1 DS)
    // → Shield pops (crusader takes 0 damage), yeti takes 3 (→ 4/2)
    let events = engine.process_action(Action::Attack {
        player: p, attacker: yeti, defender: crusader,
    }).unwrap();
    print_events("yeti into divine shield", &events);

    assert!(events.iter().any(|e| matches!(e, Event::DivineShieldPopped { .. })));
    let cru = engine.state().entities.get(&crusader).unwrap().as_minion().unwrap();
    assert_eq!(cru.health, 1, "shield absorbed: crusader still at full HP");
    let yet = engine.state().entities.get(&yeti).unwrap().as_minion().unwrap();
    assert_eq!(yet.health, 2, "yeti took 3 counter-damage → 5-3=2");
    println!("After first trade — Crusader {}/{}  Yeti {}/{}", cru.attack, cru.health, yet.attack, yet.health);
}

/// Leeroy Jenkins: Charge + Battlecry summons whelps for opponent.
#[test]
fn leeroy_jenkins_full_combo() {
    let reg = load_test_cards();
    let (mut engine, _) = GameEngine::new_game(reg, &deck_of("leeroy", 30), &deck_of("yeti", 30), rng());

    advance_to_mana(&mut engine, 5);
    let p = engine.state().active_player;
    let opp = 1 - p;

    let events = engine.process_action(Action::PlayCard {
        player: p, hand_index: 0, position: 0, target: None,
    }).unwrap();
    print_events("LEEROY JENKINS!", &events);

    // Leeroy on our board
    assert_eq!(engine.state().players[p].board.len(), 1);
    let leeroy = engine.state().players[p].board[0];
    let m = engine.state().entities.get(&leeroy).unwrap().as_minion().unwrap();
    assert_eq!((m.attack, m.health), (6, 2));

    // 2 whelps on opponent's board (battlecry)
    assert_eq!(engine.state().players[opp].board.len(), 2);
    for &eid in &engine.state().players[opp].board {
        let w = engine.state().entities.get(&eid).unwrap().as_minion().unwrap();
        assert_eq!((w.attack, w.health), (1, 1));
    }

    // Charge lets Leeroy attack immediately
    let events = engine.process_action(Action::Attack {
        player: p, attacker: leeroy, defender: GameEngine::HERO_ENTITY_ID,
    }).unwrap();
    print_events("leeroy goes face", &events);
    assert_eq!(engine.state().players[opp].hero.hp, STARTING_HP - 6);
    println!("LEEEEROY JENKINS!  Opponent at {} HP.", engine.state().players[opp].hero.hp);
}

/// Loot Hoarder dies → deathrattle draws a card for its owner.
#[test]
fn deathrattle_draw_on_death() {
    let reg = load_test_cards();
    let (mut engine, _) = GameEngine::new_game(
        reg,
        &deck_of("yeti", 30),
        &deck_of("loot", 30),
        rng(),
    );

    // Opp plays Loot Hoarder (2/1, deathrattle: draw)
    advance_to_mana(&mut engine, 4);
    let p = engine.state().active_player;
    let opp = 1 - p;

    // P plays Yeti
    engine.process_action(Action::PlayCard {
        player: p, hand_index: 0, position: 0, target: None,
    }).unwrap();

    engine.process_action(Action::EndTurn { player: p }).unwrap();
    engine.state.players[opp].mana = 10;

    engine.process_action(Action::PlayCard {
        player: opp, hand_index: 0, position: 0, target: None,
    }).unwrap();
    let hoarder = engine.state().players[opp].board[0];

    engine.process_action(Action::EndTurn { player: opp }).unwrap();

    // P's Yeti (4/5) kills Loot Hoarder (2/1)
    let yeti = engine.state().players[p].board[0];
    let opp_hand_before = engine.state().players[opp].hand.len();

    let events = engine.process_action(Action::Attack {
        player: p, attacker: yeti, defender: hoarder,
    }).unwrap();
    print_events("yeti kills loot hoarder", &events);

    assert!(events.iter().any(|e| matches!(e, Event::MinionDied { entity_id, .. } if *entity_id == hoarder)));
    assert!(events.iter().any(|e| matches!(e, Event::CardDrawn { player, .. } if *player == opp)));
    assert_eq!(engine.state().players[opp].hand.len(), opp_hand_before + 1);
    println!("Loot Hoarder died → opponent drew a card (hand {} → {})", opp_hand_before, opp_hand_before + 1);
}

/// Fatigue kills: empty deck → incrementing damage each draw.
#[test]
fn fatigue_spiral() {
    let reg = load_test_cards();
    // Tiny decks so we fatigue quickly
    let (mut engine, _) = GameEngine::new_game(reg, &deck_of("wisp", 5), &deck_of("wisp", 5), rng());

    // Decks after starting draws:  P0 drew 3+1=4, P1 drew 4 → both have 1 left
    println!("P0 deck: {}, P1 deck: {}", engine.state().players[0].deck.len(), engine.state().players[1].deck.len());

    // Burn through remaining cards by passing turns
    let mut fatigue_events = Vec::new();
    for _ in 0..30 {
        if engine.state().game_over {
            break;
        }
        let p = engine.state().active_player;
        let events = engine.process_action(Action::EndTurn { player: p }).unwrap();
        for ev in &events {
            if matches!(ev, Event::FatigueDamage { .. }) {
                fatigue_events.push(ev.clone());
            }
        }
    }

    print_events("all fatigue hits", &fatigue_events);
    assert!(engine.state().game_over, "someone should have died to fatigue");
    let winner = engine.state().winner.unwrap();
    let loser = 1 - winner;
    println!(
        "Fatigue killed Player {loser}. HP: P0={}, P1={}",
        engine.state().players[0].hero.hp,
        engine.state().players[1].hero.hp,
    );
    assert!(engine.state().players[loser].hero.hp <= 0);
}

/// Heal spell: damage hero, heal back, capped at max.
#[test]
fn heal_caps_at_max_hp() {
    let reg = load_test_cards();
    let (mut engine, _) = GameEngine::new_game(reg, &deck_of("heal8", 30), &deck_of("wisp", 30), rng());

    advance_to_mana(&mut engine, 3);
    let p = engine.state().active_player;

    // Damage ourselves a little
    engine.state.players[p].hero.hp = 25;

    let own_hero = GameEngine::hero_entity_id(p);
    engine.process_action(Action::PlayCard {
        player: p, hand_index: 0, position: 0, target: Some(own_hero),
    }).unwrap();

    // Heal 8 on 25 HP → capped at 30
    assert_eq!(engine.state().players[p].hero.hp, STARTING_HP);
    println!("Healed from 25 → {}", engine.state().players[p].hero.hp);
}

/// Arcane Intellect draws 2 cards.
#[test]
fn draw_spell_draws_two() {
    let reg = load_test_cards();
    let (mut engine, _) = GameEngine::new_game(reg, &deck_of("draw2", 30), &deck_of("wisp", 30), rng());

    advance_to_mana(&mut engine, 3);
    let p = engine.state().active_player;

    let hand = engine.state().players[p].hand.len();
    let deck = engine.state().players[p].deck.len();

    let events = engine.process_action(Action::PlayCard {
        player: p, hand_index: 0, position: 0, target: None,
    }).unwrap();
    print_events("arcane intellect", &events);

    // Played 1 card, drew 2 → net +1 hand, -2 deck
    assert_eq!(engine.state().players[p].hand.len(), hand + 1);
    assert_eq!(engine.state().players[p].deck.len(), deck - 2);
    println!("Hand: {} → {}, Deck: {} → {}",
        hand, engine.state().players[p].hand.len(),
        deck, engine.state().players[p].deck.len(),
    );
}

/// Full lethal puzzle: set up board, calculate exact lethal, kill opponent.
#[test]
fn lethal_puzzle() {
    // Scenario: opponent at 10 HP, we have Leeroy (6 charge) + Fireball (6 damage).
    // Leeroy face (6) + Fireball face (6) = 12 ≥ 10.  Exact lethal with 2 overkill.
    let reg = load_test_cards();

    // Build mixed deck: leeroy + fireballs
    let mut deck_p0 = vec!["leeroy".to_string()];
    deck_p0.extend(deck_of("fireball", 29));
    let deck_p1 = deck_of("yeti", 30);

    let (mut engine, _) = GameEngine::new_game(reg, &deck_p0, &deck_p1, rng());

    // Fast-forward: give mana, put leeroy+fireball in hand
    advance_to_mana(&mut engine, 10);
    let p = engine.state().active_player;
    let opp = 1 - p;

    engine.state.players[p].mana = 10;
    engine.state.players[opp].hero.hp = 10;
    println!("Setup: P{p} has 10 mana, opponent at 10 HP");

    // Find a Leeroy and a Fireball in hand
    let leeroy_idx = engine.state().players[p].hand.iter()
        .position(|&eid| engine.state().entities.get(&eid).map_or(false, |e| e.card_id == "leeroy"));
    let fireball_idx = engine.state().players[p].hand.iter()
        .position(|&eid| engine.state().entities.get(&eid).map_or(false, |e| e.card_id == "fireball"));

    // We need both in hand. If not, just put them there.
    if leeroy_idx.is_none() || fireball_idx.is_none() {
        // Manually arrange hand for the puzzle
        let lr_card = engine.registry().get("leeroy").unwrap().clone();
        let fb_card = engine.registry().get("fireball").unwrap().clone();

        let lr_eid = engine.state.alloc_entity_id();
        engine.state.entities.insert(lr_eid, hs_rules::entity::Entity::from_card_def(lr_eid, &lr_card, p));
        engine.state.players[p].hand.push(lr_eid);

        let fb_eid = engine.state.alloc_entity_id();
        engine.state.entities.insert(fb_eid, hs_rules::entity::Entity::from_card_def(fb_eid, &fb_card, p));
        engine.state.players[p].hand.push(fb_eid);
    }

    let hand = &engine.state().players[p].hand;
    let leeroy_idx = hand.iter()
        .position(|&eid| engine.state().entities.get(&eid).map_or(false, |e| e.card_id == "leeroy"))
        .expect("leeroy in hand");

    // Step 1: Play Leeroy (5 mana, 6/2 Charge)
    let events = engine.process_action(Action::PlayCard {
        player: p, hand_index: leeroy_idx, position: 0, target: None,
    }).unwrap();
    print_events("play leeroy", &events);
    let leeroy_eid = engine.state().players[p].board[0];

    // Step 2: Leeroy goes face (6 damage, opp drops to 4)
    let events = engine.process_action(Action::Attack {
        player: p, attacker: leeroy_eid, defender: GameEngine::HERO_ENTITY_ID,
    }).unwrap();
    print_events("leeroy face", &events);
    println!("Opponent HP after Leeroy: {}", engine.state().players[opp].hero.hp);
    assert_eq!(engine.state().players[opp].hero.hp, 4);

    // Step 3: Fireball face (6 damage, opp drops to -2)
    let hand = &engine.state().players[p].hand;
    let fb_idx = hand.iter()
        .position(|&eid| engine.state().entities.get(&eid).map_or(false, |e| e.card_id == "fireball"))
        .expect("fireball in hand");

    let opp_hero = GameEngine::hero_entity_id(opp);
    let events = engine.process_action(Action::PlayCard {
        player: p, hand_index: fb_idx, position: 0, target: Some(opp_hero),
    }).unwrap();
    print_events("fireball face — LETHAL", &events);

    assert!(engine.state().game_over);
    assert_eq!(engine.state().winner, Some(p));
    println!("LETHAL! Opponent HP: {} — Player {p} wins!", engine.state().players[opp].hero.hp);
}
