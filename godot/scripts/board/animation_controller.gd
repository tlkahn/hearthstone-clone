class_name AnimationController

const FloatingTextScene = preload("res://scenes/board/floating_text.tscn")
const BoardMinionScene = preload("res://scenes/board/board_minion.tscn")
const HandCardScene = preload("res://scenes/board/hand_card.tscn")
const FaceDownCardScene = preload("res://scenes/board/face_down_card.tscn")

var _board: Control
var _anim_layer: CanvasLayer
var animation_speed: float = 1.0
var skip_animations: bool = false


func _init(board: Control, anim_layer: CanvasLayer) -> void:
	_board = board
	_anim_layer = anim_layer


# --- Main entry point ---

func play_events(events: Array, pre_action_player: int) -> void:
	if skip_animations:
		return
	for event in events:
		await _dispatch_event(event, pre_action_player)


func _dispatch_event(event: Dictionary, pap: int) -> void:
	match event.get("event", ""):
		"attack_performed":
			await _anim_attack(event)
		"damage_dealt":
			await _anim_damage(event)
		"hero_damaged":
			await _anim_hero_damaged(event, pap)
		"divine_shield_popped":
			await _anim_divine_shield_popped(event)
		"minion_died":
			await _anim_minion_died(event)
		"minion_summoned":
			await _anim_minion_summoned(event, pap)
		"card_played":
			await _anim_card_played(event, pap)
		"card_drawn":
			await _anim_card_drawn(event, pap)
		"spell_cast":
			await _anim_spell_cast()
		"turn_started":
			await _anim_turn_started(event, pap)
		"hero_died":
			await _anim_hero_died(event, pap)
		"card_burned":
			await _anim_card_burned(event, pap)
		"fatigue_damage":
			await _anim_fatigue(event, pap)
		"weapon_equipped":
			await _anim_weapon_flash(event, pap)
		"weapon_destroyed":
			await _anim_weapon_flash(event, pap)
		"mana_spent":
			_update_mana_spent(event, pap)


# --- Utility ---

func _dur(base: float) -> float:
	return base / animation_speed


func _find_node(entity_id: int) -> Node:
	for child in _board.player_board.get_children():
		if child is BoardMinion and child._entity_id == entity_id:
			return child
	for child in _board.opponent_board.get_children():
		if child is BoardMinion and child._entity_id == entity_id:
			return child
	if _board.player_hero._entity_id == entity_id:
		return _board.player_hero
	if _board.opponent_hero._entity_id == entity_id:
		return _board.opponent_hero
	for child in _board.player_hand.get_children():
		if child is HandCard and child._entity_id == entity_id:
			return child
	return null


func _hero_for(player: int, pap: int) -> HeroPanel:
	return _board.player_hero if player == pap else _board.opponent_hero


func _board_for(player: int, pap: int) -> HBoxContainer:
	return _board.player_board if player == pap else _board.opponent_board


func _hand_for(player: int, pap: int) -> HBoxContainer:
	return _board.player_hand if player == pap else _board.opponent_hand


func _center_of(node: Control) -> Vector2:
	return node.global_position + node.size * 0.5


func _spawn_text(text: String, color: Color, target_pos: Vector2) -> void:
	var ft = FloatingTextScene.instantiate()
	_anim_layer.add_child(ft)
	ft.position = target_pos + Vector2(-40, -30)
	ft.play(text, color, 0.8)


# --- Phase 2: Combat ---

func _anim_attack(event: Dictionary) -> void:
	var attacker = _find_node(event.get("attacker", -1))
	var defender = _find_node(event.get("defender", -1))
	if not attacker or not defender:
		return

	var start = attacker.position
	var delta = (_center_of(defender) - _center_of(attacker)) * 0.7

	attacker.z_index = 20
	var tw = _board.create_tween()
	tw.tween_property(attacker, "position", start + delta, _dur(0.2))
	tw.tween_property(attacker, "position", start, _dur(0.2))
	await tw.finished
	attacker.z_index = 0


func _anim_damage(event: Dictionary) -> void:
	var target = _find_node(event.get("target", -1))
	var amount = event.get("amount", 0)
	if not target:
		return

	_spawn_text("-%d" % amount, Color.RED, _center_of(target))

	var tw = _board.create_tween()
	tw.tween_property(target, "modulate", Color(1.5, 0.5, 0.5), _dur(0.1))
	tw.tween_property(target, "modulate", Color.WHITE, _dur(0.15))
	tw.tween_interval(_dur(0.25))
	await tw.finished


func _anim_hero_damaged(event: Dictionary, pap: int) -> void:
	var hero = _hero_for(event.get("player", 0), pap)
	var amount = event.get("amount", 0)

	_spawn_text("-%d" % amount, Color.RED, _center_of(hero))

	var orig = hero.position
	var tw = _board.create_tween()
	for i in range(4):
		tw.tween_property(hero, "position", orig + Vector2(randf_range(-8, 8), randf_range(-4, 4)), _dur(0.05))
	tw.tween_property(hero, "position", orig, _dur(0.05))
	tw.tween_interval(_dur(0.25))
	await tw.finished


func _anim_divine_shield_popped(event: Dictionary) -> void:
	var node = _find_node(event.get("entity_id", -1))
	if not node:
		return

	var tw = _board.create_tween()
	tw.tween_property(node, "modulate", Color(1.0, 1.0, 0.3), _dur(0.12))
	tw.tween_property(node, "modulate", Color.WHITE, _dur(0.23))
	await tw.finished


# --- Phase 3: Summon & Death ---

func _anim_minion_summoned(event: Dictionary, pap: int) -> void:
	var player = event.get("player", 0)
	var entity_id = event.get("entity_id", -1)
	var board_pos = event.get("position", -1)
	var container = _board_for(player, pap)

	var board_data = Game.get_board(player)
	var minion_data: Dictionary = {}
	for md in board_data:
		if md.get("entity_id", -1) == entity_id:
			minion_data = md
			break

	if minion_data.is_empty():
		return

	var node = BoardMinionScene.instantiate()
	var idx = mini(board_pos, container.get_child_count())
	container.add_child(node)
	container.move_child(node, idx)
	node.set_minion_data(minion_data)
	node.minion_clicked.connect(_board._on_minion_clicked)

	node.scale = Vector2.ZERO
	node.pivot_offset = node.custom_minimum_size * 0.5

	var tw = _board.create_tween()
	tw.tween_property(node, "scale", Vector2(1.15, 1.15), _dur(0.2))
	tw.tween_property(node, "scale", Vector2.ONE, _dur(0.15))
	await tw.finished


func _anim_minion_died(event: Dictionary) -> void:
	var node = _find_node(event.get("entity_id", -1))
	if not node:
		return

	node.pivot_offset = node.size * 0.5

	var tw = _board.create_tween()
	tw.set_parallel(true)
	tw.tween_property(node, "modulate:a", 0.0, _dur(0.4))
	tw.tween_property(node, "scale", Vector2(0.8, 0.8), _dur(0.4))
	await tw.finished

	if is_instance_valid(node) and node.get_parent():
		node.get_parent().remove_child(node)
		node.queue_free()


func _anim_card_played(event: Dictionary, pap: int) -> void:
	var player = event.get("player", 0)
	var hand_index = event.get("hand_index", -1)
	var container = _hand_for(player, pap)

	var children = container.get_children()
	if hand_index < 0 or hand_index >= children.size():
		return

	var node = children[hand_index]
	node.pivot_offset = node.custom_minimum_size * 0.5

	var tw = _board.create_tween()
	tw.set_parallel(true)
	tw.tween_property(node, "scale", Vector2(0.3, 0.3), _dur(0.3))
	tw.tween_property(node, "modulate:a", 0.0, _dur(0.3))
	await tw.finished

	if is_instance_valid(node) and node.get_parent():
		node.get_parent().remove_child(node)
		node.queue_free()


# --- Phase 4: Card Draw & Spells ---

func _anim_card_drawn(event: Dictionary, pap: int) -> void:
	var player = event.get("player", 0)
	var container = _hand_for(player, pap)
	var is_local = (player == pap)

	if is_local:
		var hand = Game.get_hand(player)
		var entity_id = event.get("entity_id", -1)
		var card_data: Dictionary = {}
		for cd in hand:
			if cd.get("entity_id", -1) == entity_id:
				card_data = cd
				break
		if card_data.is_empty():
			return

		var node = HandCardScene.instantiate()
		container.add_child(node)
		node.set_card_data(card_data)
		node.hand_card_clicked.connect(_board._on_hand_card_clicked)
		node.scale = Vector2.ZERO
		node.pivot_offset = node.custom_minimum_size * 0.5

		var tw = _board.create_tween()
		tw.tween_property(node, "scale", Vector2.ONE, _dur(0.3))
		await tw.finished
	else:
		var node = FaceDownCardScene.instantiate()
		container.add_child(node)
		node.scale = Vector2.ZERO
		node.pivot_offset = node.custom_minimum_size * 0.5

		var tw = _board.create_tween()
		tw.tween_property(node, "scale", Vector2.ONE, _dur(0.3))
		await tw.finished


func _anim_spell_cast() -> void:
	var flash = ColorRect.new()
	flash.color = Color(1, 1, 1, 0.3)
	flash.size = _board.get_viewport_rect().size
	flash.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_anim_layer.add_child(flash)

	var tw = _board.create_tween()
	tw.tween_property(flash, "color:a", 0.0, _dur(0.3))
	await tw.finished
	flash.queue_free()


func _anim_card_burned(event: Dictionary, pap: int) -> void:
	var hero = _hero_for(event.get("player", 0), pap)
	_spawn_text("Burned!", Color(1.0, 0.3, 0.0), _center_of(hero))

	var tw = _board.create_tween()
	tw.tween_interval(_dur(0.4))
	await tw.finished


func _anim_fatigue(event: Dictionary, pap: int) -> void:
	var hero = _hero_for(event.get("player", 0), pap)
	var damage = event.get("damage", 0)

	_spawn_text("Fatigue! -%d" % damage, Color(1.0, 0.4, 0.0), _center_of(hero))

	var orig = hero.position
	var tw = _board.create_tween()
	for i in range(3):
		tw.tween_property(hero, "position", orig + Vector2(randf_range(-6, 6), randf_range(-3, 3)), _dur(0.05))
	tw.tween_property(hero, "position", orig, _dur(0.05))
	tw.tween_interval(_dur(0.2))
	await tw.finished


# --- Phase 5: Polish ---

func _anim_turn_started(event: Dictionary, pap: int) -> void:
	var player = event.get("player", 0)
	var text = "Your Turn" if player == pap else "Enemy Turn"

	var banner = Label.new()
	banner.text = text
	banner.add_theme_font_size_override("font_size", 48)
	banner.add_theme_color_override("font_color", Color.WHITE)
	banner.add_theme_constant_override("outline_size", 4)
	banner.add_theme_color_override("font_outline_color", Color.BLACK)
	banner.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	banner.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	banner.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_anim_layer.add_child(banner)

	var vp = _board.get_viewport_rect().size
	banner.size = Vector2(400, 80)
	banner.position = Vector2(-400, vp.y * 0.5 - 40)

	var tw = _board.create_tween()
	tw.tween_property(banner, "position:x", vp.x * 0.5 - 200, _dur(0.2))
	tw.tween_interval(_dur(0.4))
	tw.tween_property(banner, "modulate:a", 0.0, _dur(0.2))
	await tw.finished
	banner.queue_free()


func _anim_hero_died(event: Dictionary, pap: int) -> void:
	var hero = _hero_for(event.get("player", 0), pap)
	var orig = hero.position

	hero.modulate = Color(2.0, 0.2, 0.2)

	var tw = _board.create_tween()
	for i in range(6):
		tw.tween_property(hero, "position", orig + Vector2(randf_range(-12, 12), randf_range(-6, 6)), _dur(0.05))
	tw.tween_property(hero, "position", orig, _dur(0.05))
	tw.tween_property(hero, "modulate", Color.WHITE, _dur(0.25))
	await tw.finished


func _anim_weapon_flash(event: Dictionary, pap: int) -> void:
	var hero = _hero_for(event.get("player", 0), pap)

	var tw = _board.create_tween()
	tw.tween_property(hero, "modulate", Color(1.0, 1.0, 0.5), _dur(0.1))
	tw.tween_property(hero, "modulate", Color.WHITE, _dur(0.2))
	await tw.finished


func _update_mana_spent(event: Dictionary, pap: int) -> void:
	var remaining = event.get("remaining", 0)
	var mana = Game.get_mana(pap)
	_board.mana_label.text = "Mana: %d/%d" % [remaining, mana.get("max", 0)]
