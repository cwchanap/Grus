extends Node3D

## The simulation owns the one `RUINS` table. Imported art has a unit-square
## base, scaled to each bridge-derived inclusive rectangle without overhang.
## Ruins are static terrain presentation only: no entities, no gameplay.

const MODELS := [
	preload("res://assets/winter/collapsed_building.glb"),
	preload("res://assets/winter/wrecked_vehicle.glb"),
	preload("res://assets/winter/frozen_debris.glb"),
]

func _ready() -> void:
	var payload: PackedInt32Array = GrusBridge.obstacle_rects()
	assert(
		payload.size() % 4 == 0,
		"obstacle_rects must be flat inclusive min/max quads"
	)
	for index in range(0, payload.size(), 4):
		var min_cell := Vector2i(payload[index], payload[index + 1])
		var max_cell := Vector2i(payload[index + 2], payload[index + 3])
		var cells := max_cell - min_cell + Vector2i.ONE
		var ruin: Node3D = MODELS[(index / 4) % MODELS.size()].instantiate()
		ruin.name = "Ruin%d" % (index / 4)
		ruin.scale = Vector3(cells.x, 1.0, cells.y)
		ruin.position = Vector3(
			min_cell.x + cells.x * 0.5,
			0.0,
			min_cell.y + cells.y * 0.5
		)
		add_child(ruin)
