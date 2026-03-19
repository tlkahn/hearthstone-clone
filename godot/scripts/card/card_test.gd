extends Control

@onready var card_container: HBoxContainer = $CardContainer

const CardDisplayScene = preload("res://scenes/card/card_display.tscn")


func _ready() -> void:
	# Wait a frame for CardDB autoload to finish loading
	await get_tree().process_frame

	var ids = CardDB.get_all_card_ids()
	print("CardDB has %d cards" % CardDB.get_card_count())

	for id in ids:
		var data: Dictionary = CardDB.get_card(id)
		var card_node: Control = CardDisplayScene.instantiate()
		card_container.add_child(card_node)
		# set_card_data must be called after the node enters the tree
		# so @onready vars are populated
		card_node.set_card_data(data)
		print("  Rendered: %s (%s)" % [data.get("name", "?"), id])


func _input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed:
		if event.keycode == KEY_F5:
			CardDB.reload_cards()
			# Clear and re-render
			for child in card_container.get_children():
				child.queue_free()
			_ready()
