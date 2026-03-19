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
│   ├── rules/                  # hs-rules crate — card types, loader, registry
│   │   └── src/
│   │       ├── card.rs         # CardDef, CardSet, CardTypeData, Keyword, Rarity, EffectTag
│   │       ├── card_loader.rs  # CardRegistry, load_from_directory, validation
│   │       └── lib.rs
│   └── gdext-bridge/           # hs-gdext-bridge crate — GodotClasses
│       └── src/
│           ├── card_bridge.rs  # CardDatabase (autoload as CardDB)
│           └── lib.rs          # GDExtension entry point
├── data/cards/                 # Card definitions in RON format
├── godot/                      # Godot project root (res:// resolves here)
│   ├── project.godot
│   ├── hearthstone.gdextension
│   ├── scenes/card/            # Card display and test scenes
│   ├── scripts/card/           # card_display.gd, card_test.gd
│   └── assets/art/             # Placeholder frames and card art
└── doc/
    ├── metaplan.md             # High-level plan — 7 major systems
    └── card-data-pipeline.md   # System 1 implementation doc
```

## Build & Test

```bash
cargo build                     # Build all crates
cargo test -p hs-rules          # Run 17 unit/integration tests
cargo build -p hs-gdext-bridge  # Build the Godot extension dylib
```

Open `godot/project.godot` in Godot to run the game. Main test scene: `scenes/card/card_test.tscn`.

## System Status

| # | System | Status | Key files |
|---|--------|--------|-----------|
| 1 | Card Data Pipeline | **Done** | `crates/rules/src/card.rs`, `card_loader.rs`, `gdext-bridge/src/card_bridge.rs` |
| 2 | Game Rules Engine | Not started | `crates/rules/` (will extend) |
| 3 | Board UI & Interaction | Not started | `godot/scenes/board/`, `godot/scripts/board/` |
| 4 | Animations & VFX | Not started | `godot/scenes/vfx/`, `godot/scripts/vfx/` |
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
- **Deps:** `serde` + `ron 0.8` + `thiserror 2` for rules; `godot 0.4` for bridge

## Key Design Decisions

- **RON over JSON** for card definitions (enum support, comments, trailing commas)
- **`CardTypeData` tagged enum** enforces that minions have attack/health, weapons have attack/durability, spells have neither — compiler-checked
- **`EffectTag(String)`** is an opaque placeholder for future effect system hookup
- **`crates/rules` is pure Rust** with zero Godot dependency — testable independently, reusable by server
- **CardDB autoload** — globally accessible in GDScript via `CardDB.get_card(id)`
