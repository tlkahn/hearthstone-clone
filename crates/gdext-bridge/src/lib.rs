use godot::prelude::*;

mod card_bridge;

struct HearthstoneExtension;

#[gdextension]
unsafe impl ExtensionLibrary for HearthstoneExtension {}
