@tool
extends EditorScenePostImport

## Post-import for the HPA-290 winter props: each GLB ships one shared
## material and carries its colors as COLOR_0 vertex colors, so every prop
## renders as a single surface. Godot does not honor COLOR_0 unless the
## material opts in, which has no .import toggle — enable it here.

func _post_import(scene: Node) -> Object:
	_enable_vertex_color_albedo(scene)
	return scene

func _enable_vertex_color_albedo(node: Node) -> void:
	var mesh_instance := node as MeshInstance3D
	if mesh_instance != null and mesh_instance.mesh != null:
		for surface in mesh_instance.mesh.get_surface_count():
			var material := mesh_instance.get_active_material(surface) as StandardMaterial3D
			if material != null:
				material.vertex_color_use_as_albedo = true
	for child in node.get_children():
		_enable_vertex_color_albedo(child)
