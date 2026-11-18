extends RefCounted

## Shared presentation helpers for the HUD and underground drawer: icon
## lookup cache, currency order/formatting, affordability, and integer grouping.

const CURRENCIES := ["food", "scrap", "coal", "crystals"]
const CURRENCY_SHORT := {
	"food": "F",
	"scrap": "S",
	"coal": "C",
	"crystals": "Cr",
}

static var _icon_cache: Dictionary = {}

static func icon_for_kind(group: String, kind: String) -> Texture2D:
	var path := "res://assets/ui/icons/%s/%s.png" % [group, kind.to_snake_case()]
	if not _icon_cache.has(path):
		_icon_cache[path] = ResourceLoader.load(path) as Texture2D if ResourceLoader.exists(path) else null
	return _icon_cache[path] as Texture2D

static func inline_icon(group: String, kind: String, size := 18) -> String:
	var icon := icon_for_kind(group, kind)
	if icon == null:
		return ""
	return "[img=%dx%d alt=\"%s\"]%s[/img] " % [
		size, size, kind.capitalize(), icon.resource_path]

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
	for resource in CURRENCIES:
		var amount := int(cost.get(resource, 0))
		if amount > 0:
			parts.append("%d %s" % [amount, resource.capitalize()])
	return ", ".join(parts) if not parts.is_empty() else "free"

static func cost_short(cost: Dictionary) -> String:
	var parts := PackedStringArray()
	for resource in CURRENCIES:
		var amount := int(cost.get(resource, 0))
		if amount > 0:
			parts.append("%d%s" % [amount, str(CURRENCY_SHORT[resource])])
	return " ".join(parts) if not parts.is_empty() else "free"

static func cost_markup(cost: Dictionary, icon_size := 15) -> String:
	var parts := PackedStringArray()
	for resource in CURRENCIES:
		var amount := int(cost.get(resource, 0))
		if amount > 0:
			parts.append("%s%d" % [inline_icon("resources", resource.capitalize(), icon_size).strip_edges(), amount])
	return " ".join(parts)

static func can_afford(cost: Dictionary, economy: Dictionary) -> bool:
	for resource in CURRENCIES:
		if int(cost.get(resource, 0)) > int(economy.get(resource, 0)):
			return false
	return true

static func format_int(value: int) -> String:
	var raw := str(value)
	var out := ""
	var count := 0
	for i in range(raw.length() - 1, -1, -1):
		if count > 0 and count % 3 == 0 and raw[i] != "-":
			out = "," + out
		out = raw[i] + out
		count += 1
	return out
