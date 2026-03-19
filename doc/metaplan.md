# Hearthstone-Clone Metaplan

> **Stack:** Godot 4 + gdext (Rust) | **Target:** Online 1v1 card battler with collection
> **Scope:** Solo/small-team indie — "Hearthstone-lite", not a full live-service CCG

---

## Architecture Split

- **GDScript side:** UI scenes, animations, tweens, input handling, visual effects
- **Rust (gdext) side:** Game rules engine, card effect resolution, state validation, network protocol, deck validation

This split plays to each language's strength — Godot for presentation, Rust for correctness.

---

## Core Skill Areas

| Area               | What's needed                                                        |
| ------------------- | -------------------------------------------------------------------- |
| **Game design**     | Card mechanics, balance, mana curve, keyword system                  |
| **Godot + gdext**   | Scene trees, UI (Control nodes), signals, GDScript glue + Rust core  |
| **Networking**      | Authoritative server for competitive play (cheating prevention)      |
| **Backend**         | Accounts, matchmaking, card collection, deck storage                 |
| **2D Art**          | Card frames, illustrations, board, UI elements, VFX                  |
| **Audio**           | SFX (card play, attack, spells), ambient, music                      |

---

## Major Systems

### 1. Card Data Pipeline

- Card definitions (JSON/RON) — stats, keywords, effects
- Card rendering scene (template that populates from data)

### 2. Game Rules Engine (Rust)

- Turn structure, mana, zones (hand / board / deck / graveyard)
- Effect/ability system (battlecry, deathrattle, auras, triggers)
- Targeting system
- Stack/priority resolution
- **This is the hardest part.**

### 3. Board UI & Interaction

- Hand fanning, card hover/zoom
- Drag-to-play, drag-to-attack
- Targeting arrows
- Board slot management

### 4. Animations & VFX

- Card play, attack, damage, death, spell cast
- Tweens, particle effects, screen shake
### 5. Networking

- Client-server protocol (WebSocket or TCP)
- Authoritative server (Rust standalone, reusing the rules engine crate)
- State sync, reconnection

### 6. Backend Services

- Auth, matchmaking, collection/deck storage
- Options: self-hosted (Rust + PostgreSQL) or BaaS (Supabase, Nakama)

### 7. Menus & Meta-Game

- Collection viewer, deck builder, matchmaking lobby
- Heavily UI — Godot's Control nodes shine here
---

## Asset Budget (Minimum Viable)

| Asset                | Quantity                     | Source options                              |
| -------------------- | ---------------------------- | ------------------------------------------ |
| Card illustrations   | 80–120 for a base set        | AI-generated, commissioned, or asset packs |
| Card frame template  | 3–5 (minion, spell, weapon)  | Design yourself or commission              |
| Board + UI art       | ~20 elements                 | Asset stores, freelance                    |
| SFX                  | ~30–50 clips                 | Freesound, asset packs                     |
| Music                | 3–5 tracks                   | Royalty-free libraries, commission         |

---

## Biggest Risks

1. **The effect system** — Hearthstone's card interactions are combinatorially complex. Scope keyword count aggressively for v1 (start with ~5 keywords, not 30).
2. **Networking** — Turn-based helps, but authoritative server + reconnection + state sync is still substantial.
3. **Art volume** — Each card needs unique art. This is often the bottleneck for solo devs. Consider a unified art style that's fast to produce.

---

## Recommended Starting Point

Build the **rules engine in Rust as a standalone crate** first, with unit tests for card interactions. Then wrap it with gdext for the Godot client and reuse it in the server binary. This lets you validate game mechanics before touching any UI.

```
hearthstone-clone/
├── crates/
│   ├── rules/          # Pure Rust — game logic, card effects, state machine
│   ├── server/         # Rust — authoritative game server, reuses rules crate
│   └── gdext-bridge/   # gdext — exposes rules to Godot
├── godot/
│   ├── scenes/         # Card, board, menus
│   ├── scripts/        # GDScript glue
│   └── assets/         # Art, audio
├── data/
│   └── cards/          # Card definitions (JSON/RON)
└── doc/
    └── metaplan.md     # This file
```
