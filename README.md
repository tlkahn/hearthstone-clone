# Hearthstone Clone

A Hearthstone-like 2D card game built with **Godot 4.6** and **Rust** (via [gdext](https://github.com/godot-rust/gdextension)).

Game logic runs in pure Rust for correctness and testability; Godot handles UI, animations, and input.

## Status

| System | Status |
|--------|--------|
| Card Data Pipeline | Done |
| Game Rules Engine | Done |
| Board UI & Interaction | Done |
| Animations & VFX | Done |
| Networking | Not started |
| Backend Services | Not started |
| Menus & Meta-Game | Not started |

## Architecture

```
GDScript (UI, animations, input)
    ↕  gdext FFI bridge
Rust (game rules, validation, card data)
```

- **`crates/rules`** — pure Rust, zero Godot dependency. Game engine, card definitions (RON format), effect system, targeting. 80+ unit tests.
- **`crates/gdext-bridge`** — thin bridge exposing Rust types to Godot as GodotClasses.
- **`godot/`** — scenes, scripts, assets. Board UI, hand/minion interaction, attack/spell animations with VFX and SFX.

## Build

```bash
cargo build                     # Build all crates
cargo test -p hs-rules          # Run unit/integration tests
cargo build -p hs-gdext-bridge  # Build the Godot extension
```

Open `godot/project.godot` in Godot 4.6 to run the game.

## Key Design Decisions

- **RON over JSON** for card definitions — enum support, comments, trailing commas
- **Action/Event sourcing** — player intents produce event streams; enables networking, animation, and replay
- **Pure Rust rules engine** — testable independently, reusable by a future authoritative server
- **Effect system** — 6 effect types with a targeting system (TargetSpec + TargetFilter), depth-limited recursion for deathrattle chains

## License

Godot Engine is MIT-licensed. This project is not affiliated with Blizzard Entertainment.
