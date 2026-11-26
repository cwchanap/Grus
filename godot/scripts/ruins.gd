extends Node3D

## The simulation owns the one `RUINS` table. Imported art has a unit-square
## base, scaled to each bridge-derived inclusive rectangle without overhang.
## Ruins are static terrain presentation only: no entities, no gameplay.

## Ruins in cells the player has never explored stay hidden: the fog overlay
## sits just above the ground, so a tall ruin would otherwise poke out of the
## dark and reveal terrain the player has not scouted.
var _ruin_rects: Array[Rect2i] = []
var _revision := -2

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
		_ruin_rects.append(Rect2i(min_cell, cells))

func _process(_delta: float) -> void:
	var revision := int(GrusBridge.visibility_revision())
	if revision == _revision:
		return
	var snap: Dictionary = {}
	if revision >= 0:
		snap = GrusBridge.visibility_snapshot()
		if int(snap.get("revision", -1)) != revision:
			return
	_revision = revision
	var width := int(snap.get("width", 0))
	var states: PackedInt32Array = snap.get("states", PackedInt32Array())
	for index in _ruin_rects.size():
		var ruin := get_child(index) as Node3D
		# No visibility state (visibility-free benchmark): everything shows.
		ruin.visible = revision < 0 or states.size() == 0 \
				or _any_explored(states, width, _ruin_rects[index])

func _any_explored(states: PackedInt32Array, width: int, rect: Rect2i) -> bool:
	for y in range(rect.position.y, rect.end.y):
		for x in range(rect.position.x, rect.end.x):
			var cell := y * width + x
			if cell >= 0 and cell < states.size() and states[cell] != 0:
				return true
	return false
