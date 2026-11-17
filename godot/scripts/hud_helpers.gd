extends RefCounted

## Shared presentation helpers for the HUD and underground drawer: one icon
## lookup cache and one four-currency cost formatter.

static var _icon_cache: Dictionary = {}

static func icon_for_kind(group: String, kind: String) -> Texture2D:
	var path := "res://assets/ui/icons/%s/%s.png" % [group, kind.to_snake_case()]
	if not _icon_cache.has(path):
		_icon_cache[path] = ResourceLoader.load(path) as Texture2D if ResourceLoader.exists(path) else null
	return _icon_cache[path] as Texture2D

## The catalogue's producer building kinds, in one place for the controller
## (rally/train context) and the HUD (train-row filter): a unit with no
## producer contributes nothing, so "" can never pass `_producer_kinds.has`.
static func producer_kinds(units: Dictionary) -> Array[String]:
	var kinds: Array[String] = []
	for kind in units:
		var producer := str(units[kind].get("producer", ""))
		if producer != "" and not kinds.has(producer):
			kinds.append(producer)
	return kinds

static func cost_text(cost: Dictionary) -> String:
	var parts := PackedStringArray()
	for resource in ["food", "scrap", "coal", "crystals"]:
		var amount := int(cost.get(resource, 0))
		if amount > 0:
			parts.append("%d %s" % [amount, resource.capitalize()])
	return ", ".join(parts) if not parts.is_empty() else "free"
