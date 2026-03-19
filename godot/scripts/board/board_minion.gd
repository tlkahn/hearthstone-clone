extends PanelContainer
class_name BoardMinion

signal minion_clicked(entity_id: int)

@onready var name_label: Label = $VBox/NameLabel
@onready var stats_label: RichTextLabel = $VBox/StatsLabel
@onready var keywords_label: Label = $VBox/KeywordsLabel
@onready var sickness_overlay: ColorRect = $SicknessOverlay
@onready var taunt_border: Panel = $TauntBorder

var _entity_id: int = -1
var _can_attack: bool = false


func set_minion_data(data: Dictionary) -> void:
	_entity_id = data.get("entity_id", -1)
	_can_attack = data.get("can_attack", false)

	var mname: String = data.get("name", "???")
	if mname.length() > 12:
		mname = mname.left(11) + "…"
	name_label.text = mname

	var atk: int = data.get("attack", 0)
	var hp: int = data.get("health", 0)
	var max_hp: int = data.get("max_health", hp)
	if hp < max_hp:
		stats_label.text = "[center]%d / [color=red]%d[/color][/center]" % [atk, hp]
	else:
		stats_label.text = "[center]%d / %d[/center]" % [atk, hp]

	var kws = data.get("keywords", [])
	var kw_parts: PackedStringArray = []
	var has_taunt := false
	var has_shield := false
	for kw in kws:
		match str(kw):
			"taunt":
				has_taunt = true
				kw_parts.append("T")
			"divine_shield":
				has_shield = true
				kw_parts.append("DS")
			"charge":
				kw_parts.append("C")
			"deathrattle":
				kw_parts.append("DR")
			"battlecry":
				kw_parts.append("BC")
	keywords_label.text = " ".join(kw_parts) if kw_parts.size() > 0 else ""

	taunt_border.visible = has_taunt

	var sick: bool = data.get("summoning_sickness", false)
	sickness_overlay.visible = sick


func set_selected(value: bool) -> void:
	if value:
		add_theme_stylebox_override("panel", _make_style(Color(1.0, 1.0, 0.0, 0.4)))
	else:
		_clear_highlight()


func set_targetable(value: bool) -> void:
	if value:
		add_theme_stylebox_override("panel", _make_style(Color(0.2, 0.8, 0.2, 0.4)))
	else:
		_clear_highlight()


func set_attackable(value: bool) -> void:
	if value and _can_attack:
		add_theme_stylebox_override("panel", _make_style(Color(0.3, 0.9, 0.3, 0.3)))
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
		minion_clicked.emit(_entity_id)
