extends PanelContainer
class_name HandCard

signal hand_card_clicked(hand_index: int)

@onready var card_display: Control = $CardDisplay
@onready var unplayable_overlay: ColorRect = $UnplayableOverlay

var _hand_index: int = -1
var _playable: bool = false
var _selected: bool = false


func _ready() -> void:
	_disable_mouse_recursive(card_display)


func set_card_data(data: Dictionary) -> void:
	_hand_index = data.get("hand_index", -1)
	_playable = data.get("playable", false)

	# Forward card data to the embedded CardDisplay
	card_display.set_card_data(data)

	unplayable_overlay.visible = not _playable


func set_selected(value: bool) -> void:
	_selected = value
	if value:
		# Move up when selected
		position.y = -20
		add_theme_stylebox_override("panel", _make_style(Color(1.0, 1.0, 0.0, 0.4)))
	else:
		position.y = 0
		_clear_highlight()


func set_playable_highlight(value: bool) -> void:
	if value and _playable:
		add_theme_stylebox_override("panel", _make_style(Color(0.2, 0.8, 0.2, 0.3)))
	else:
		_clear_highlight()


func _clear_highlight() -> void:
	remove_theme_stylebox_override("panel")


func _make_style(color: Color) -> StyleBoxFlat:
	var style = StyleBoxFlat.new()
	style.bg_color = color
	style.set_corner_radius_all(4)
	return style


func _gui_input(event: InputEvent) -> void:
	if event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		hand_card_clicked.emit(_hand_index)


func _on_mouse_entered() -> void:
	if not _selected:
		z_index = 10
		scale = Vector2(1.05, 1.05)


func _on_mouse_exited() -> void:
	if not _selected:
		z_index = 0
		scale = Vector2(1.0, 1.0)


func _disable_mouse_recursive(node: Node) -> void:
	if node is Control:
		node.mouse_filter = Control.MOUSE_FILTER_IGNORE
	for child in node.get_children():
		_disable_mouse_recursive(child)
