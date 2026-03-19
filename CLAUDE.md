# Hearthstone Clone

A Hearthstone-like 2D card game built with **Godot 4.6 + gdext (Rust)**.

## Stack

- **Godot 4.6** — game engine, UI, scenes, animations
- **Rust via gdext 0.4** — game logic, validation, server (future)
- **RON** — card data format (not JSON)
- **License:** Godot is MIT, no fees/royalties

## Architecture

- **GDScript** — UI scenes, animations, tweens, input, visual effects
- **Rust (`crates/rules`)** — pure Rust, no Godot dependency. Game logic, card data, validation. Independently testable, reusable by server.
- **Rust (`crates/gdext-bridge`)** — thin bridge exposing Rust types to Godot as GodotClasses. Returns `VarDictionary` (not the deprecated `Dictionary`).

## Project Structure

```
hearthstone-clone/
├── Cargo.toml                  # Workspace: crates/rules, crates/gdext-bridge
├── crates/
│   ├── rules/                  # hs-rules crate — card types, loader, game engine
│   │   └── src/
│   │       ├── card.rs         # CardDef, CardSet, CardTypeData, Keyword, Rarity
│   │       ├── card_loader.rs  # CardRegistry, load_from_directory, validation
│   │       ├── types.rs        # EntityId, PlayerId, constants
│   │       ├── entity.rs       # Entity (runtime card instance), MinionEntity, WeaponEntity
│   │       ├── game_state.rs   # GameState, Player, Hero
│   │       ├── action.rs       # Action enum (EndTurn, PlayCard, Attack)
│   │       ├── event.rs        # Event enum (CardDrawn, DamageDealt, MinionDied, etc.)
│   │       ├── error.rs        # GameError enum
│   │       ├── engine.rs       # GameEngine — central orchestrator
│   │       ├── effect.rs       # Effect enum, TargetSpec, TargetFilter
│   │       ├── effect_exec.rs  # Effect execution, targeting, deathrattle processing
│   │       └── lib.rs
│   └── gdext-bridge/           # hs-gdext-bridge crate — GodotClasses
│       └── src/
│           ├── card_bridge.rs  # CardDatabase (autoload as CardDB)
│           ├── game_bridge.rs  # GameBridge (autoload — game actions/state)
│           └── lib.rs          # GDExtension entry point
├── data/cards/                 # Card definitions in RON format
├── godot/                      # Godot project root (res:// resolves here)
│   ├── project.godot
│   ├── hearthstone.gdextension
│   ├── scenes/card/            # Card display and test scenes
│   ├── scenes/board/           # Board UI scenes (board_scene, hero_panel, etc.)
│   ├── scripts/card/           # card_display.gd, card_test.gd
│   ├── scripts/board/          # board_scene.gd, hero_panel.gd, etc.
│   └── assets/art/             # Placeholder frames and card art
└── doc/
    ├── metaplan.md             # High-level plan — 7 major systems
    ├── card-data-pipeline.md   # System 1 implementation doc
    ├── game-rules-engine.md    # System 2 implementation doc
    ├── board-ui-interaction.md # System 3 implementation doc
    └── animations-vfx.md      # System 4 implementation doc
```

## Build & Test

```bash
cargo build                     # Build all crates
cargo test -p hs-rules          # Run 87 unit/integration tests
cargo build -p hs-gdext-bridge  # Build the Godot extension dylib
```

Open `godot/project.godot` in Godot to run the game. Main scene: `scenes/board/board_scene.tscn`. Card test scene: `scenes/card/card_test.tscn`.

## System Status

| # | System | Status | Key files |
|---|--------|--------|-----------|
| 1 | Card Data Pipeline | **Done** | `crates/rules/src/card.rs`, `card_loader.rs`, `gdext-bridge/src/card_bridge.rs` |
| 2 | Game Rules Engine | **Done** | `crates/rules/src/engine.rs`, `effect.rs`, `effect_exec.rs`, `game_state.rs`, `entity.rs` |
| 3 | Board UI & Interaction | **Done** | `gdext-bridge/src/game_bridge.rs`, `godot/scenes/board/`, `godot/scripts/board/` |
| 4 | Animations & VFX | **Done** | `godot/scripts/board/animation_controller.gd`, `floating_text.gd`, `godot/scenes/board/floating_text.tscn` |
| 5 | Networking | Not started | `crates/network/`, `crates/server/` |
| 6 | Backend Services | Not started | `crates/server/` |
| 7 | Menus & Meta-Game | Not started | `godot/scenes/menus/`, `godot/scripts/menus/` |

## Conventions

- **TDD workflow** — write failing test first, make it pass, then refactor
- **Card data in RON** — one file per card set under `data/cards/`, loaded by `CardRegistry::load_from_directory`
- **gdext bridge returns `VarDictionary`** — not the deprecated `Dictionary` type alias
- **GDScript type inference** — use `var x = CardDB.method()` (not `:=`) for gdext return values since types can't be inferred across FFI
- **Hot-reload** — press F5 in-game to reload card data from disk without restarting
- **Docs in Obsidian Markdown** — system docs live in `doc/` using wikilinks, callouts, Mermaid diagrams
- **Deps:** `serde` + `ron 0.8` + `thiserror 2` + `rand 0.8` for rules; `godot 0.4` for bridge

## Key Design Decisions

- **RON over JSON** for card definitions (enum support, comments, trailing commas)
- **`CardTypeData` tagged enum** enforces that minions have attack/health, weapons have attack/durability, spells have neither — compiler-checked
- **`Effect` enum** with 6 types (DealDamage, Heal, Summon, DrawCards, BuffMinion, DestroyMinion) and targeting system (TargetSpec + TargetFilter)
- **Action/Event sourcing** — actions are player intents, events are atomic state mutations; enables networking, animation, replay
- **`crates/rules` is pure Rust** with zero Godot dependency — testable independently, reusable by server
- **CardDB autoload** — globally accessible in GDScript via `CardDB.get_card(id)`
