extends Label
class_name FloatingText


func play(text_value: String, color: Color, duration: float = 0.8) -> void:
	text = text_value
	add_theme_color_override("font_color", color)
	add_theme_font_size_override("font_size", 28)
	add_theme_constant_override("outline_size", 3)
	add_theme_color_override("font_outline_color", Color.BLACK)
	horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	mouse_filter = MOUSE_FILTER_IGNORE

	var tween = create_tween()
	tween.set_parallel(true)
	tween.tween_property(self, "position:y", position.y - 60, duration)
	tween.tween_property(self, "modulate:a", 0.0, duration * 0.4).set_delay(duration * 0.6)
	tween.set_parallel(false)
	tween.tween_callback(queue_free)
