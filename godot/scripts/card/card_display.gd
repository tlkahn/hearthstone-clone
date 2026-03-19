extends Control
class_name CardDisplay

const ART_BASE_PATH := "res://assets/art/cards/"
const PLACEHOLDER_ART := "res://assets/art/cards/placeholder.png"

const FRAME_TEXTURES := {
	"minion": preload("res://assets/art/frames/minion_frame.png"),
	"spell": preload("res://assets/art/frames/spell_frame.png"),
	"weapon": preload("res://assets/art/frames/weapon_frame.png"),
}

@onready var card_frame: TextureRect = $CardFrame
@onready var artwork: TextureRect = $Artwork
@onready var mana_cost_label: Label = $ManaCost
@onready var card_name_label: Label = $CardName
@onready var card_text_label: RichTextLabel = $CardText
@onready var attack_icon: Control = $AttackIcon
@onready var attack_label: Label = $AttackIcon/AttackLabel
@onready var health_icon: Control = $HealthIcon
@onready var health_label: Label = $HealthIcon/HealthLabel
@onready var rarity_gem: TextureRect = $RarityGem


func set_card_data(data: Dictionary) -> void:
	if data.is_empty():
		return

	card_name_label.text = data.get("name", "???")
	mana_cost_label.text = str(data.get("mana_cost", 0))

	var raw_text: String = data.get("text", "")
	card_text_label.text = _format_card_text(raw_text)

	var card_type: String = data.get("card_type", "minion")
	if FRAME_TEXTURES.has(card_type):
		card_frame.texture = FRAME_TEXTURES[card_type]

	match card_type:
		"minion":
			attack_icon.visible = true
			health_icon.visible = true
			attack_label.text = str(data.get("attack", 0))
			health_label.text = str(data.get("health", 0))
		"weapon":
			attack_icon.visible = true
			health_icon.visible = true
			attack_label.text = str(data.get("attack", 0))
			health_label.text = str(data.get("durability", 0))
		"spell":
			attack_icon.visible = false
			health_icon.visible = false

	var art_file: String = data.get("art", "")
	var art_path := ART_BASE_PATH + art_file
	if ResourceLoader.exists(art_path):
		artwork.texture = load(art_path)
	else:
		artwork.texture = load(PLACEHOLDER_ART)

	_set_rarity_gem(data.get("rarity", "free"))


func _format_card_text(raw: String) -> String:
	var result := raw
	for kw in ["Battlecry", "Deathrattle", "Taunt", "Charge", "Divine Shield"]:
		result = result.replace(kw, "[b]%s[/b]" % kw)
	return result


func _set_rarity_gem(rarity: String) -> void:
	var color: Color
	match rarity:
		"free":
			rarity_gem.visible = false
			return
		"common":
			color = Color.WHITE
		"rare":
			color = Color.CORNFLOWER_BLUE
		"epic":
			color = Color.MEDIUM_PURPLE
		"legendary":
			color = Color.ORANGE
		_:
			color = Color.WHITE
	rarity_gem.visible = true
	rarity_gem.modulate = color
