extends Node3D

## Bridge-derived ruin meshes: the simulation owns the one `RUINS` table —
## this script reads `GrusBridge.obstacle_rects()` once and stamps one
## primitive BoxMesh per authored rectangle. Each payload quad is a grid
## rectangle's minimum and INCLUSIVE maximum cell; centers and sizes are
## derived here, never hardcoded — no second coordinate list may exist.
## Ruins are static terrain presentation only: no entities, no gameplay.

const RUIN_HEIGHT := 2.0

func _ready() -> void:
	var payload: PackedInt32Array = GrusBridge.obstacle_rects()
	assert(
		payload.size() % 4 == 0,
		"obstacle_rects must be flat inclusive min/max quads"
	)
	var material := StandardMaterial3D.new()
	material.albedo_color = Color(0.28, 0.29, 0.31, 1.0)
	material.roughness = 0.9
	for index in range(0, payload.size(), 4):
		var min_cell := Vector2i(payload[index], payload[index + 1])
		var max_cell := Vector2i(payload[index + 2], payload[index + 3])
		var cells := max_cell - min_cell + Vector2i.ONE
		var mesh := BoxMesh.new()
		mesh.size = Vector3(cells.x, RUIN_HEIGHT, cells.y)
		var ruin := MeshInstance3D.new()
		ruin.name = "Ruin%d" % (index / 4)
		ruin.mesh = mesh
		ruin.material_override = material
		ruin.position = Vector3(
			min_cell.x + cells.x * 0.5,
			RUIN_HEIGHT * 0.5,
			min_cell.y + cells.y * 0.5
		)
		add_child(ruin)
