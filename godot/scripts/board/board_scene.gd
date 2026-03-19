extends Control

# --- Preloads ---
const HandCardScene = preload("res://scenes/board/hand_card.tscn")
const FaceDownCardScene = preload("res://scenes/board/face_down_card.tscn")
const BoardMinionScene = preload("res://scenes/board/board_minion.tscn")

# --- Interaction state machine ---
enum InteractionState { IDLE, CARD_SELECTED, ATTACKER_SELECTED, TARGETING_SPELL }

var _state: InteractionState = InteractionState.IDLE
var _selected_hand_index: int = -1
var _selected_attacker_id: int = -1
var _valid_targets: Array = []
var _animating: bool = false
var _anim_controller: AnimationController

# --- Node references ---
@onready var player_hand: HBoxContainer = $PlayerArea/PlayerHand
@onready var opponent_hand: HBoxContainer = $OpponentArea/OpponentHand
@onready var player_board: HBoxContainer = $PlayerArea/PlayerBoard
@onready var opponent_board: HBoxContainer = $OpponentArea/OpponentBoard
@onready var player_hero: PanelContainer = $PlayerArea/PlayerBottomRow/PlayerHero
@onready var opponent_hero: PanelContainer = $OpponentArea/OpponentTopRow/OpponentHero
@onready var mana_label: Label = $PlayerArea/PlayerBottomRow/ManaLabel
@onready var end_turn_button: Button = $CenterRow/EndTurnButton
@onready var turn_label: Label = $CenterRow/TurnLabel
@onready var status_label: Label = $StatusLabel
@onready var game_over_panel: PanelContainer = $GameOverPanel
@onready var game_over_label: Label = $GameOverPanel/VBox/GameOverLabel
@onready var restart_button: Button = $GameOverPanel/VBox/RestartButton
@onready var debug_panel: VBoxContainer = $DebugPanel
@onready var debug_info: Label = $DebugPanel/DebugInfo
@onready var animation_layer: CanvasLayer = $AnimationLayer


func _ready() -> void:
	game_over_panel.visible = false
	end_turn_button.pressed.connect(_on_end_turn_pressed)
	restart_button.pressed.connect(_on_restart_pressed)
	_anim_controller = AnimationController.new(self, animation_layer)

	await get_tree().process_frame
	_start_new_game()


func _start_new_game() -> void:
	var deck_p0 = _build_test_deck()
	var deck_p1 = _build_test_deck()
	Game.start_game(deck_p0, deck_p1)
	game_over_panel.visible = false
	_set_state(InteractionState.IDLE)
	refresh_ui()


func _build_test_deck() -> Array[String]:
	var deck: Array[String] = []
	for i in range(4):
		deck.append("basic_wisp")
	for i in range(6):
		deck.append("basic_river_crocolisk")
	for i in range(6):
		deck.append("basic_chillwind_yeti")
	for i in range(4):
		deck.append("basic_senjin_shieldmasta")
	for i in range(4):
		deck.append("basic_boulderfist_ogre")
	for i in range(6):
		deck.append("basic_fireball")
	return deck


# --- Animation ---

func _play_events(events: Array, pre_action_player: int) -> void:
	_animating = true
	end_turn_button.disabled = true
	await _anim_controller.play_events(events, pre_action_player)
	_animating = false


# --- UI Refresh ---

func refresh_ui() -> void:
	var active = Game.get_active_player()
	_render_hand(active, player_hand, false)
	var opp = 1 - active
	_render_hand(opp, opponent_hand, true)
	_render_board(active, player_board, true)
	_render_board(opp, opponent_board, false)
	_render_hero(active, player_hero)
	_render_hero(opp, opponent_hero)
	_update_mana_display(active)
	_update_turn_indicator()
	_update_debug_info()
	_highlight_playable_cards()
	_highlight_attackable_minions()
	_check_game_over()


func _render_hand(player: int, container: HBoxContainer, face_down: bool) -> void:
	_clear_children(container)

	if face_down:
		var hand = Game.get_hand(player)
		for card_data in hand:
			var card_back = FaceDownCardScene.instantiate()
			container.add_child(card_back)
	else:
		var hand = Game.get_hand(player)
		for card_data in hand:
			var card_node = HandCardScene.instantiate()
			container.add_child(card_node)
			card_node.set_card_data(card_data)
			card_node.hand_card_clicked.connect(_on_hand_card_clicked)


func _render_board(player: int, container: HBoxContainer, is_active: bool) -> void:
	_clear_children(container)

	var board = Game.get_board(player)
	for minion_data in board:
		var minion_node = BoardMinionScene.instantiate()
		container.add_child(minion_node)
		minion_node.set_minion_data(minion_data)
		minion_node.minion_clicked.connect(_on_minion_clicked)


func _render_hero(player: int, panel: PanelContainer) -> void:
	var hero_data = Game.get_hero(player)
	panel.set_hero_data(hero_data)


func _update_mana_display(player: int) -> void:
	var mana = Game.get_mana(player)
	mana_label.text = "Mana: %d/%d" % [mana.get("current", 0), mana.get("max", 0)]


func _update_turn_indicator() -> void:
	var turn = Game.get_turn_number()
	var active = Game.get_active_player()
	turn_label.text = "Turn %d — Player %d" % [turn, active + 1]
	end_turn_button.disabled = false


func _update_debug_info() -> void:
	var active = Game.get_active_player()
	var opp = 1 - active
	debug_info.text = "P%d deck: %d | P%d deck: %d" % [
		active + 1, Game.get_deck_size(active),
		opp + 1, Game.get_deck_size(opp)
	]


func _highlight_playable_cards() -> void:
	if _state != InteractionState.IDLE:
		return
	for card in player_hand.get_children():
		if card is HandCard:
			card.set_playable_highlight(true)


func _highlight_attackable_minions() -> void:
	if _state != InteractionState.IDLE:
		return
	for minion in player_board.get_children():
		if minion is BoardMinion:
			minion.set_attackable(true)


func _check_game_over() -> void:
	if Game.is_game_over():
		var winner = Game.get_winner()
		if winner >= 0:
			game_over_label.text = "Player %d Wins!" % [winner + 1]
		else:
			game_over_label.text = "Draw!"
		game_over_panel.visible = true


func _clear_children(node: Node) -> void:
	for child in node.get_children():
		child.queue_free()


# --- State Machine ---

func _set_state(new_state: InteractionState) -> void:
	_state = new_state
	_clear_all_highlights()
	match new_state:
		InteractionState.IDLE:
			_selected_hand_index = -1
			_selected_attacker_id = -1
			_valid_targets = []
			_highlight_playable_cards()
			_highlight_attackable_minions()
		InteractionState.CARD_SELECTED:
			_highlight_selected_card()
		InteractionState.TARGETING_SPELL:
			_highlight_selected_card()
			_highlight_valid_targets()
		InteractionState.ATTACKER_SELECTED:
			_highlight_selected_attacker()
			_highlight_valid_attack_targets()


func _clear_all_highlights() -> void:
	for card in player_hand.get_children():
		if card is HandCard:
			card.set_selected(false)
			card.set_playable_highlight(false)
	for minion in player_board.get_children():
		if minion is BoardMinion:
			minion.set_selected(false)
			minion.set_targetable(false)
			minion.set_attackable(false)
	for minion in opponent_board.get_children():
		if minion is BoardMinion:
			minion.set_targetable(false)
	player_hero.set_targetable(false)
	player_hero.set_selected(false)
	opponent_hero.set_targetable(false)


func _highlight_selected_card() -> void:
	for card in player_hand.get_children():
		if card is HandCard and card._hand_index == _selected_hand_index:
			card.set_selected(true)


func _highlight_valid_targets() -> void:
	for tid in _valid_targets:
		_set_entity_targetable(tid, true)


func _highlight_selected_attacker() -> void:
	for minion in player_board.get_children():
		if minion is BoardMinion and minion._entity_id == _selected_attacker_id:
			minion.set_selected(true)


func _highlight_valid_attack_targets() -> void:
	for tid in _valid_targets:
		_set_entity_targetable(tid, true)


func _set_entity_targetable(entity_id: int, value: bool) -> void:
	# Check opponent board
	for minion in opponent_board.get_children():
		if minion is BoardMinion and minion._entity_id == entity_id:
			minion.set_targetable(value)
			return
	# Check player board (for friendly-target spells)
	for minion in player_board.get_children():
		if minion is BoardMinion and minion._entity_id == entity_id:
			minion.set_targetable(value)
			return
	# Check heroes
	if opponent_hero._entity_id == entity_id:
		opponent_hero.set_targetable(value)
	elif player_hero._entity_id == entity_id:
		player_hero.set_targetable(value)


# --- Input Handlers ---

func _on_hand_card_clicked(hand_index: int) -> void:
	if _animating:
		return
	match _state:
		InteractionState.IDLE:
			# Check if the card at this index is playable
			var hand = Game.get_hand(Game.get_active_player())
			if hand_index >= 0 and hand_index < hand.size():
				var card_data = hand[hand_index]
				if not card_data.get("playable", false):
					_show_status("Not enough mana!")
					return

				_selected_hand_index = hand_index
				if Game.needs_target(hand_index):
					_valid_targets = Array(Game.get_valid_targets(hand_index))
					if _valid_targets.is_empty():
						# No valid targets — play without target
						_play_card_action(hand_index, -1)
					else:
						_set_state(InteractionState.TARGETING_SPELL)
				else:
					# Non-targeted card — play directly (minion or non-targeted spell)
					_play_card_action(hand_index, -1)
		_:
			# Clicking hand card from other states → cancel
			_set_state(InteractionState.IDLE)


func _on_minion_clicked(entity_id: int) -> void:
	if _animating:
		return
	match _state:
		InteractionState.IDLE:
			# Check if this is our minion that can attack
			if Game.can_attack(entity_id):
				_selected_attacker_id = entity_id
				_valid_targets = Array(Game.get_valid_attack_targets(entity_id))
				_set_state(InteractionState.ATTACKER_SELECTED)
		InteractionState.ATTACKER_SELECTED:
			if entity_id == _selected_attacker_id:
				# Clicked same minion — deselect
				_set_state(InteractionState.IDLE)
			elif _valid_targets.has(entity_id):
				# Attack this target
				_attack_action(_selected_attacker_id, entity_id)
			else:
				# Check if it's another of our minions that can attack
				if Game.can_attack(entity_id):
					_selected_attacker_id = entity_id
					_valid_targets = Array(Game.get_valid_attack_targets(entity_id))
					_set_state(InteractionState.ATTACKER_SELECTED)
				else:
					_set_state(InteractionState.IDLE)
		InteractionState.TARGETING_SPELL:
			if _valid_targets.has(entity_id):
				_play_card_action(_selected_hand_index, entity_id)
			else:
				_set_state(InteractionState.IDLE)
		InteractionState.CARD_SELECTED:
			_set_state(InteractionState.IDLE)


func _on_hero_clicked(entity_id: int) -> void:
	if _animating:
		return
	match _state:
		InteractionState.ATTACKER_SELECTED:
			if _valid_targets.has(entity_id):
				_attack_action(_selected_attacker_id, entity_id)
			else:
				_set_state(InteractionState.IDLE)
		InteractionState.TARGETING_SPELL:
			if _valid_targets.has(entity_id):
				_play_card_action(_selected_hand_index, entity_id)
			else:
				_set_state(InteractionState.IDLE)
		_:
			_set_state(InteractionState.IDLE)


func _on_end_turn_pressed() -> void:
	if _animating:
		return
	var pre_active = Game.get_active_player()
	var result = Game.end_turn()
	if result.get("ok", false):
		var events = result.get("events", [])
		_set_state(InteractionState.IDLE)
		await _play_events(events, pre_active)
		refresh_ui()
	else:
		_show_status(result.get("error", "Unknown error"))


func _on_restart_pressed() -> void:
	_start_new_game()


# --- Actions ---

func _play_card_action(hand_index: int, target_id: int) -> void:
	var pre_active = Game.get_active_player()
	var board_size = Game.get_board(pre_active).size()
	var result = Game.play_card(hand_index, board_size, target_id)
	if result.get("ok", false):
		var events = result.get("events", [])
		_set_state(InteractionState.IDLE)
		await _play_events(events, pre_active)
		refresh_ui()
	else:
		_show_status(result.get("error", "Unknown error"))
		_set_state(InteractionState.IDLE)


func _attack_action(attacker_id: int, defender_id: int) -> void:
	var pre_active = Game.get_active_player()
	var result = Game.attack(attacker_id, defender_id)
	if result.get("ok", false):
		var events = result.get("events", [])
		_set_state(InteractionState.IDLE)
		await _play_events(events, pre_active)
		refresh_ui()
	else:
		_show_status(result.get("error", "Unknown error"))
		_set_state(InteractionState.IDLE)


# --- Status Messages ---

func _show_status(msg: String) -> void:
	status_label.text = msg
	status_label.visible = true
	var tween = create_tween()
	tween.tween_interval(2.0)
	tween.tween_callback(func(): status_label.visible = false)


# --- Global Input ---

func _unhandled_input(event: InputEvent) -> void:
	if _animating:
		return
	if event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_RIGHT:
		_set_state(InteractionState.IDLE)
	if event is InputEventKey and event.pressed and event.keycode == KEY_ESCAPE:
		_set_state(InteractionState.IDLE)
