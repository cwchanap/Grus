@tool
extends EditorScenePostImport

func _post_import(scene: Node) -> Object:
	_set_palette_filter(scene)
	return scene

func _set_palette_filter(node: Node) -> void:
	if node is MeshInstance3D and node.mesh != null:
		for surface in node.mesh.get_surface_count():
			var material := node.get_active_material(surface) as StandardMaterial3D
			if material != null:
				material.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST
	for child in node.get_children():
		_set_palette_filter(child)
