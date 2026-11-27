extends Node3D

## The bridge pushes building state on first sight and transitions: `complete`
## flips the shared painted construction-site sprite for the kind's finished
## presentation — a painted Sprite3D for eight kinds, the retained GLB
## PackedScene for Wall/Gate — exactly once, `cold` toggles a reversible icy
## tint on sprite kinds. Neither flag is polled — Rust is authoritative — and
## pushes arriving before the stamped building_kind metadata just cache until
## _process first sees it.
const SITE_TEXTURE := preload("res://assets/structures/painted/construction_site.png")
const PAINTED := {
	"ResearchCenter": preload("res://assets/structures/painted/research_center.png"),
	"Bunkhouse": preload("res://assets/structures/painted/bunkhouse.png"),
	"Depot": preload("res://assets/structures/painted/depot.png"),
	"Barracks": preload("res://assets/structures/painted/barracks.png"),
	"GunTurret": preload("res://assets/structures/painted/gun_turret.png"),
	"CoalFurnace": preload("res://assets/structures/painted/coal_furnace.png"),
	"FlameTower": preload("res://assets/structures/painted/flame_tower.png"),
	"CrystalLance": preload("res://assets/structures/painted/crystal_lance.png"),
}
## Finished Wall/Gate keep their 3D models for the 1×1 seam/corridor
## contract; they never render a sprite.
const MODELS := {
	"Wall": preload("res://assets/structures/wall.glb"),
	"Gate": preload("res://assets/structures/gate.glb"),
}
## Sprite presentation: the quad pitches to match the active Camera3D's view
## tilt (default 54° when no camera is active) and its bottom image edge sits on
## the camera-side (+z) footprint edge a hair above the ground (the painted PNG
## already carries its baked contact shadow, so the sun must not cast a second
## one).
const DEFAULT_SPRITE_PITCH_DEGREES := 54.0
const GROUND_LIFT := 0.01
## Shared alpha threshold for the opaque-pixel cache (health bar seat and
## silhouette picking read the same mask).
const ALPHA_THRESHOLD := 0.1
## Cold reads as a subtle reversible icy tint on painted sprite kinds;
## warm returns to WHITE. Wall/Gate GLBs never take it.
const COLD_TINT := Color(0.72, 0.83, 1.0, 1.0)

@onready var health_bar: MeshInstance3D = $HealthBar

## One shared per-texture opaque-pixel cache (HPA-697): health-bar seating
## here and camera-ray hit-testing in battlefield_controller both read the
## same thresholded BitMap instead of scanning raw pixels per query.
static var _alpha_masks := {}
static var _opaque_tops := {}
var _styled_kind := ""
var _complete := false
var _cold := false
var _health_ratio := 1.0
var _sprite_root_scale := Vector2.ZERO

func _ready() -> void:
	add_to_group("building_views")
	set_notify_transform(true)

## The valid art geometry under a model for bounds/picking scanners: a
## mesh with geometry or a textured Sprite3D — both expose get_aabb().
## Sprite3D specifically, never the wider SpriteBase3D: AnimatedSprite3D
## carries sprite_frames, not texture, so a base-class read script-errors
## instead of filtering the node out.
static func art_geometry(node: Node) -> GeometryInstance3D:
	if node is MeshInstance3D:
		var mesh_instance := node as MeshInstance3D
		return null if mesh_instance.mesh == null else mesh_instance
	if node is Sprite3D:
		var sprite := node as Sprite3D
		return null if sprite.texture == null else sprite
	return null

## Whether geometry under `model` is actually rendered: the geometry node
## and every ancestor through `model` itself must be visible — a hidden
## mesh still carries triangles and bounds but supplies no pixels. The view
## root's own visibility stays the caller's call: fog hides live views and
## standalone probes inspect views whose root was never shown.
static func art_rendered(art: GeometryInstance3D, model: Node) -> bool:
	var ancestor: Node = art
	var stop := model.get_parent()
	while ancestor != null and ancestor != stop:
		if ancestor is Node3D and not (ancestor as Node3D).visible:
			return false
		ancestor = ancestor.get_parent()
	return true

## Thresholded opaque mask for a texture, cached per file-backed texture
## resource (anonymous runtime ImageTextures stay uncached so transient test
## textures are not pinned until process exit).
static func alpha_mask(texture: Texture2D) -> BitMap:
	if texture == null:
		return null
	var cacheable := not texture.resource_path.is_empty()
	if cacheable and _alpha_masks.has(texture):
		return _alpha_masks[texture]
	var image := texture.get_image()
	if image == null:
		return null
	if image.is_compressed():
		image.decompress()
	var mask := BitMap.new()
	mask.create_from_image_alpha(image, ALPHA_THRESHOLD)
	if cacheable:
		_alpha_masks[texture] = mask
	return mask

## Topmost opaque row's horizontal center, in texture pixel coordinates
## (x = column, y = row from the top) — the visually opaque artwork top,
## not the full quad top with its near-transparent halo pixels. Returns
## Vector2.INF when the texture has no opaque pixel at all.
static func opaque_top_pixels(texture: Texture2D) -> Vector2:
	if texture == null:
		return Vector2.INF
	var cacheable := not texture.resource_path.is_empty()
	if cacheable and _opaque_tops.has(texture):
		return _opaque_tops[texture]
	var top := Vector2.INF
	var mask := alpha_mask(texture)
	if mask != null:
		var size := mask.get_size()
		for row in int(size.y):
			var first := -1
			var last := -1
			for col in int(size.x):
				if mask.get_bitv(Vector2i(col, row)):
					if first < 0:
						first = col
					last = col
			if first >= 0:
				top = Vector2((first + last) * 0.5, row)
				break
	if cacheable:
		_opaque_tops[texture] = top
	return top

## Sprite-local 3D point of a texture pixel (coordinates as returned by
## opaque_top_pixels; image rows run top-down, sprite-local +y is up). A
## horizontally flipped sprite (painted units facing left) mirrors columns.
static func sprite_local_from_pixel(sprite: SpriteBase3D, pixel: Vector2) -> Vector3:
	var rect := sprite.get_item_rect()
	var col := rect.size.x - pixel.x if sprite.flip_h else pixel.x
	return Vector3(
		sprite.pixel_size * (rect.position.x + col),
		sprite.pixel_size * (rect.position.y + rect.size.y - pixel.y),
		0.0)

## Texture pixel under a sprite-local point (the picking ray's triangle
## hit), image rows top-down. Returns Vector2.INF outside the drawn rect.
static func sprite_pixel_from_local(sprite: SpriteBase3D, local: Vector3) -> Vector2:
	if sprite.pixel_size <= 0.0:
		return Vector2.INF
	var rect := sprite.get_item_rect()
	var col := local.x / sprite.pixel_size - rect.position.x
	if sprite.flip_h:
		col = rect.size.x - col
	var row := rect.size.y - (local.y / sprite.pixel_size - rect.position.y)
	if col < 0.0 or row < 0.0 or col >= rect.size.x or row >= rect.size.y:
		return Vector2.INF
	return Vector2(col, row)

## Whether the texel at a texture pixel coordinate is opaque under the
## shared threshold — the same mask that seats the health bar.
static func pixel_opaque(texture: Texture2D, pixel: Vector2) -> bool:
	if texture == null or pixel == Vector2.INF:
		return false
	var mask := alpha_mask(texture)
	if mask == null:
		return false
	var col := int(pixel.x)
	var row := int(pixel.y)
	var size := mask.get_size()
	if col < 0 or row < 0 or col >= size.x or row >= size.y:
		return false
	return mask.get_bitv(Vector2i(col, row))

func _process(_delta: float) -> void:
	if not has_meta("building_kind"):
		return
	_styled_kind = str(get_meta("building_kind"))
	# The kind meta is stamped once at spawn and never changes, and every
	# later state change arrives through set_state — style once, then stop
	# polling every frame.
	if _attach_model():
		set_process(false)

## Rust value-gates authoritative state pushes. These flags also preserve
## early state and make repeated completion or cold pushes idempotent.
func set_state(complete: bool, cold: bool) -> void:
	var finishing := complete and not _complete
	_complete = complete
	if cold != _cold:
		_cold = cold
		_apply_cold()
	if _styled_kind == "":
		# Pushed before building_kind metadata was stamped (or before
		# _process first saw it): the cached flags drive the first
		# _attach_model, so a seeded-complete building renders its final
		# model without ever flashing the site.
		return
	if finishing:
		set_process(not _attach_model())

func _attach_model() -> bool:
	var instance: Node = null
	if not _complete:
		instance = _painted_sprite_node(SITE_TEXTURE)
	elif PAINTED.has(_styled_kind):
		instance = _painted_sprite_node(PAINTED[_styled_kind])
	elif MODELS.has(_styled_kind):
		_sprite_root_scale = Vector2.ZERO
		instance = (MODELS[_styled_kind] as PackedScene).instantiate()
	else:
		_sprite_root_scale = Vector2.ZERO
		# A kind whose art has not landed yet must still render and be
		# debuggable — an invisible building would silently stand.
		push_warning("building_view: no model for building_kind '%s', using placeholder" % _styled_kind)
		instance = _placeholder()
	if instance == null:
		# The bridge root transform (footprint scale) has not landed yet;
		# keep polling and retry the attach once it has.
		return false
	var model := instance as Node3D
	if model == null:
		instance.free()
		push_error("building_view: failed to instantiate model for '%s'" % _styled_kind)
		return false
	var previous := get_node_or_null("Model")
	if previous != null:
		remove_child(previous)
		previous.queue_free()
	model.name = "Model"
	add_child(model)
	_apply_cold()
	_seat_health_bar()
	return true

func _sprite_pitch_degrees() -> float:
	var viewport := get_viewport()
	var cam := viewport.get_camera_3d() if viewport != null else null
	if cam != null:
		return clampf(rad_to_deg(atan2(cam.global_basis.z.y, cam.global_basis.z.z)), 0.0, 90.0)
	return DEFAULT_SPRITE_PITCH_DEGREES

func _apply_painted_sprite_layout(holder: Node3D, sprite: Sprite3D, width: float, depth: float, pitch_deg: float) -> void:
	var pitch_rad := deg_to_rad(pitch_deg)
	holder.scale = Vector3(1.0 / width, 1.0, 1.0 / depth)
	sprite.pixel_size = width / float(sprite.texture.get_width())
	sprite.rotation_degrees = Vector3(-pitch_deg, 0.0, 0.0)
	sprite.position = Vector3(
		0.0,
		0.5 * width * cos(pitch_rad) + GROUND_LIFT,
		0.5 * depth - 0.5 * width * sin(pitch_rad))

func _notification(what: int) -> void:
	if what != NOTIFICATION_TRANSFORM_CHANGED or _sprite_root_scale == Vector2.ZERO:
		return
	var width := scale.x
	var depth := scale.z
	if width <= 0.001 or depth <= 0.001 or not is_finite(width) or not is_finite(depth):
		return
	if is_equal_approx(width, _sprite_root_scale.x) and is_equal_approx(depth, _sprite_root_scale.y):
		return
	var model := get_node_or_null("Model") as Node3D
	if model == null:
		return
	var sprites := model.find_children("*", "Sprite3D", true, false)
	if sprites.is_empty():
		return
	var sprite := sprites[0] as Sprite3D
	if sprite == null or sprite.texture == null:
		return
	_apply_painted_sprite_layout(model, sprite, width, depth, _sprite_pitch_degrees())
	_sprite_root_scale = Vector2(width, depth)
	if model.has_meta("_cached_art_bounds"):
		model.remove_meta("_cached_art_bounds")
	_seat_health_bar()

## Build the painted-sprite presentation under a wrapper that cancels the
## Rust root's nonuniform X/Z footprint scale, so the sprite plane renders
## width world units wide (pixel_size = width / texture width) and stays
## square. It pitches to face the active camera with billboard off, and
## the wrapper-local center coordinates equal world-relative ones because
## the wrapper and root scales cancel exactly. Returns null while the root
## scale is not yet usable (the attach retries next _process).
func _painted_sprite_node(texture: Texture2D) -> Node3D:
	var width := scale.x
	var depth := scale.z
	if width <= 0.001 or depth <= 0.001 or not is_finite(width) or not is_finite(depth):
		return null
	var holder := Node3D.new()
	var sprite := Sprite3D.new()
	sprite.texture = texture
	sprite.billboard = BaseMaterial3D.BILLBOARD_DISABLED
	sprite.shaded = false
	sprite.alpha_cut = SpriteBase3D.ALPHA_CUT_OPAQUE_PREPASS
	sprite.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	sprite.modulate = COLD_TINT if _cold else Color.WHITE
	_apply_painted_sprite_layout(holder, sprite, width, depth, _sprite_pitch_degrees())
	_sprite_root_scale = Vector2(width, depth)
	holder.add_child(sprite)
	return holder

func _placeholder() -> Node3D:
	var holder := Node3D.new()
	var mesh_instance := MeshInstance3D.new()
	var box := BoxMesh.new()
	box.size = Vector3(0.8, 1.0, 0.8)
	mesh_instance.mesh = box
	mesh_instance.position.y = 0.5
	var material := StandardMaterial3D.new()
	material.albedo_color = Color(0.62, 0.67, 0.72, 1.0)
	material.roughness = 0.85
	mesh_instance.material_override = material
	holder.add_child(mesh_instance)
	return holder

## Cold reaches only the painted sprites, as the shared reversible icy
## tint (warm == WHITE); the retained Wall/Gate GLBs keep their plain
## shading — Rust never marks them cold and they must not gain a fake
## frost effect.
func _apply_cold() -> void:
	var model := get_node_or_null("Model") as Node3D
	if model == null:
		return
	for node in model.find_children("*", "SpriteBase3D", true, false):
		(node as SpriteBase3D).modulate = COLD_TINT if _cold else Color.WHITE

## The bar seats just above the active model's real visual top each time
## the model swaps: painted kinds use the shared opaque-pixel cache — the
## topmost opaque texture row's horizontal center, never the transparent
## halo the raw quad top or Image.get_used_rect() would give — converted
## from sprite-local into this view's local space (the root scales X/Z by
## the footprint, so world-space coordinates must not be written directly).
## Retained Wall/Gate GLBs keep the mesh AABB path.
func _seat_health_bar() -> void:
	var model := get_node_or_null("Model") as Node3D
	if model == null:
		return
	var sprites := model.find_children("*", "Sprite3D", true, false)
	if not sprites.is_empty():
		var sprite := sprites[0] as Sprite3D
		var top := opaque_top_pixels(sprite.texture)
		if top == Vector2.INF:
			return
		var view_local: Vector3 = global_transform.affine_inverse() \
				* (sprite.global_transform * sprite_local_from_pixel(sprite, top))
		health_bar.position = Vector3(view_local.x, view_local.y + 0.15, view_local.z)
		return
	var bounds := AABB()
	var first := true
	for node in model.find_children("*", "MeshInstance3D", true, false):
		var mesh := node as MeshInstance3D
		var local: AABB = global_transform.affine_inverse() * mesh.global_transform * mesh.get_aabb()
		bounds = local if first else bounds.merge(local)
		first = false
	if first:
		return
	health_bar.position = Vector3(bounds.get_center().x, bounds.end.y + 0.15, bounds.get_center().z)

## Called from the Rust side on Changed<Health>; the bar appears and shrinks
## once a building takes damage. Full health stays hidden.
func set_health_ratio(ratio: float) -> void:
	_health_ratio = clampf(ratio, 0.0, 1.0)
	health_bar.scale.x = maxf(_health_ratio, 0.02)
	health_bar.visible = _health_ratio < 0.999

func health_ratio() -> float:
	return _health_ratio
