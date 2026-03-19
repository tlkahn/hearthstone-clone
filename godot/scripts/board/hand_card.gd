extends PanelContainer
class_name HandCard

signal hand_card_clicked(hand_index: int)

@onready var mana_label: Label = $VBox/ManaLabel
@onready var name_label: Label = $VBox/NameLabel
@onready var type_label: Label = $VBox/TypeLabel
@onready var stats_label: Label = $VBox/StatsLabel
@onready var text_label: RichTextLabel = $VBox/TextLabel
@onready var highlight_rect: ColorRect = $HighlightRect
@onready var unplayable_overlay: ColorRect = $UnplayableOverlay

var _hand_index: int = -1
var _entity_id: int = -1
var _playable: bool = false
var _selected: bool = false


func set_card_data(data: Dictionary) -> void:
	_entity_id = data.get("entity_id", -1)
	_hand_index = data.get("hand_index", -1)
	_playable = data.get("playable", false)

	mana_label.text = "(%d)" % data.get("mana_cost", 0)

	var cname: String = data.get("name", "???")
	if cname.length() > 14:
		cname = cname.left(13) + "…"
	name_label.text = cname

	var card_type: String = data.get("card_type", "minion")
	match card_type:
		"minion":
			type_label.text = "Minion"
			stats_label.text = "%d / %d" % [data.get("attack", 0), data.get("health", 0)]
			stats_label.visible = true
		"spell":
			type_label.text = "Spell"
			stats_label.visible = false
		"weapon":
			type_label.text = "Weapon"
			stats_label.text = "%d / %d" % [data.get("attack", 0), data.get("durability", 0)]
			stats_label.visible = true

	var raw_text: String = data.get("text", "")
	if raw_text.length() > 0:
		text_label.text = "[center]%s[/center]" % raw_text
		text_label.visible = true
	else:
		text_label.visible = false

	unplayable_overlay.visible = not _playable


func set_selected(value: bool) -> void:
	_selected = value
	if value:
		position.y = -20
		highlight_rect.color = Color(1.0, 1.0, 0.0, 0.4)
		highlight_rect.visible = true
	else:
		position.y = 0
		highlight_rect.visible = false


func set_playable_highlight(value: bool) -> void:
	if value and _playable:
		highlight_rect.color = Color(0.2, 0.8, 0.2, 0.3)
		highlight_rect.visible = true
	else:
		highlight_rect.visible = false


func _gui_input(event: InputEvent) -> void:
	if event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		hand_card_clicked.emit(_hand_index)


func _on_mouse_entered() -> void:
	if not _selected:
		z_index = 10
		scale = Vector2(1.1, 1.1)


func _on_mouse_exited() -> void:
	if not _selected:
		z_index = 0
		scale = Vector2(1.0, 1.0)
