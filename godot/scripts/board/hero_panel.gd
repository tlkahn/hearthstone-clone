extends PanelContainer
class_name HeroPanel

signal hero_clicked(entity_id: int)

@onready var hp_label: Label = $VBox/HPLabel
@onready var armor_label: Label = $VBox/ArmorLabel
@onready var weapon_label: Label = $VBox/WeaponLabel
@onready var name_label: Label = $VBox/NameLabel

var _entity_id: int = -1
var _targetable: bool = false
var _selected: bool = false


func set_hero_data(data: Dictionary) -> void:
	_entity_id = data.get("entity_id", -1)
	var hp: int = data.get("hp", 0)
	var max_hp: int = data.get("max_hp", 30)
	var armor: int = data.get("armor", 0)

	hp_label.text = "HP: %d/%d" % [hp, max_hp]

	if armor > 0:
		armor_label.text = "Armor: %d" % armor
		armor_label.visible = true
	else:
		armor_label.visible = false

	if data.has("weapon"):
		var wep: Dictionary = data.get("weapon")
		weapon_label.text = "Weapon: %d/%d" % [wep.get("attack", 0), wep.get("durability", 0)]
		weapon_label.visible = true
	else:
		weapon_label.visible = false


func set_targetable(value: bool) -> void:
	_targetable = value
	if value:
		add_theme_stylebox_override("panel", _make_style(Color(0.2, 0.8, 0.2, 0.3)))
	else:
		_clear_highlight()


func set_selected(value: bool) -> void:
	_selected = value
	if value:
		add_theme_stylebox_override("panel", _make_style(Color(1.0, 1.0, 0.0, 0.3)))
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
		hero_clicked.emit(_entity_id)
