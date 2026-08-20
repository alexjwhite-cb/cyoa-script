# cyoa_engine.gd — GDScript wrapper for the CYOA engine.
#
# Since GDScript cannot call native C functions directly, this script
# delegates to the C# CyoaEngine class (bindings/godot/csharp/CyoaEngine.cs).
# The C# wrapper handles DllImport of the native C-ABI and exposes a
# Godot-friendly API.
#
# This class supports two usage patterns:
#   1. Engine mode — load a story and play it:
#      var engine = preload("res://addons/cyoa/cyoa_engine.gd").new()
#      engine.load_from_path("res://addons/cyoa/native/forest_adventure.cyoa.bc")
#      print(engine.get_current_event_text())
#      var choices = engine.get_current_choices()
#      engine.make_choice(0)
#
#   2. Catalog mode — register multiple stories and filter by tags:
#      var catalog = preload("res://addons/cyoa/cyoa_engine.gd").new()
#      catalog.register_story("res://addons/cyoa/native/forest_adventure.cyoa.bc")
#      catalog.register_story("res://addons/cyoa/native/tavern_tales.cyoa.bc")
#      var fantasy_stories = catalog.stories_with_tag("fantasy")
#      var engine = catalog.create_engine_by_name("ForestAdventure")
#      print(engine.get_current_event_text())
#
# Requires: Godot 4.x with .NET support (Godot.NET).
# The native plugin (libcyoa_native.so / cyoa_native.dll / libcyoa_native.dylib)
# must be in the project's `addons/cyoa/native/` directory.

@tool
extends RefCounted
class_name CyoaEngineGD

# ── Engine / Catalog state ──────────────────────────────────────────────────────

# In engine mode: a C# CyoaEngine instance.
# In catalog mode: a C# CyoaStoryCatalog instance.
var _engine: Object
var _catalog: Object

# Whether this instance is acting as a catalog (vs a single engine).
var _is_catalog: bool

func _init():
	_engine = null
	_catalog = null
	_is_catalog = false

# ── Catalog mode ───────────────────────────────────────────────────────────────

func register_story(path: String) -> void:
	"""Register a compiled story from a .cyoa.bc file path. Only valid in catalog mode."""
	if not _is_catalog:
		_catalog = preload("res://addons/cyoa/scripts/CyoaEngine.cs").new()
		_is_catalog = true
	_engine = null  # can't be both engine and catalog
	var bytes = FileAccess.get_file_as_bytes(path)
	_catalog.RegisterStory(bytes)

func register_story_from_bytes(bytes: PackedVectorByteArray) -> void:
	"""Register a compiled story from bytecode bytes. Only valid in catalog mode."""
	if not _is_catalog:
		_catalog = preload("res://addons/cyoa/scripts/CyoaEngine.cs").new()
		_is_catalog = true
	_engine = null
	_catalog.RegisterStory(bytes)

func story_count() -> int:
	"""Number of registered stories. Only valid in catalog mode."""
	return _catalog.StoryCount if _catalog != null else 0

func list_stories() -> Array:
	"""List all registered stories. Only valid in catalog mode."""
	if _catalog == null:
		return []
	var stories = _catalog.ListStories()
	var result = []
	for s in stories:
		result.append({"name": s.Name, "tags": s.Tags})
	return result

func stories_with_tag(tag: String) -> Array:
	"""Find all stories that have a specific tag. Only valid in catalog mode."""
	if _catalog == null:
		return []
	var stories = _catalog.StoriesWithTag(tag)
	var result = []
	for s in stories:
		result.append({"name": s.Name, "tags": s.Tags})
	return result

func stories_with_all_tags(tags: Array) -> Array:
	"""Find all stories that have ALL of the specified tags."""
	if _catalog == null:
		return []
	var stories = _catalog.StoriesWithAllTags(tags)
	var result = []
	for s in stories:
		result.append({"name": s.Name, "tags": s.Tags})
	return result

func stories_with_any_tags(tags: Array) -> Array:
	"""Find all stories that have ANY of the specified tags."""
	if _catalog == null:
		return []
	var stories = _catalog.StoriesWithAnyTags(tags)
	var result = []
	for s in stories:
		result.append({"name": s.Name, "tags": s.Tags})
	return result

func create_engine(index: int) -> Object:
	"""Create an engine for the story at the given catalog index."""
	if _catalog == null:
		return null
	var cs_engine = _catalog.CreateEngine(index)
	var gd_engine = preload("res://addons/cyoa/cyoa_engine.gd").new()
	gd_engine._set_native_engine(cs_engine)
	return gd_engine

func create_engine_by_name(name: String) -> Object:
	"""Find a story by name and create an engine for it."""
	if _catalog == null:
		return null
	var cs_engine = _catalog.CreateEngineByName(name)
	var gd_engine = preload("res://addons/cyoa/cyoa_engine.gd").new()
	gd_engine._set_native_engine(cs_engine)
	return gd_engine

# ── Engine mode ────────────────────────────────────────────────────────────────

func _set_native_engine(native_engine: Object) -> void:
	"""Internal: set the native C# engine from a catalog's CreateEngine call."""
	_engine = native_engine
	_is_catalog = false
	_catalog = null

# Load a story from bytecode bytes (alternative to LoadFromFile).
func load_from_bytes(bytecode: PackedVectorByteArray) -> Error:
	if bytecode.is_empty():
		return ERR_INVALID_INPUT
	_engine = preload("res://addons/cyoa/scripts/CyoaEngine.cs").new(bytecode)
	_is_catalog = false
	_catalog = null
	return OK if _engine != null else ERR_LOAD_SCRIPTS_FAILED

# Load a story from a .cyoa.bc file path.
func load_from_path(path: String) -> Error:
	if not FileAccess.file_exists(path):
		return ERR_FILE_NOT_FOUND
	var bytes = FileAccess.get_file_as_bytes(path)
	if bytes.is_empty():
		return ERR_FILE_CORRUPT_UTF8
	return load_from_bytes(bytes)

# Current event's internal ID (name).
func get_current_event_id() -> String:
	return _engine.CurrentEventId ?? "" if _engine != null else ""

# Current event text — paragraphs joined by newlines.
func get_current_event_text() -> String:
	return _engine.CurrentEventText ?? "" if _engine != null else ""

# Number of currently available choices.
func get_choice_count() -> int:
	return _engine.ChoiceCount if _engine != null else 0

# Get the text of a choice at the given index.
func get_choice_text(index: int) -> String:
	return _engine.GetChoiceText(index) ?? "" if _engine != null else ""

# Get all current choices as an array.
func get_current_choices() -> PackedStringArray:
	if _engine == null:
		return []
	return PackedStringArray(_engine.CurrentChoices)

# Apply the player's choice at the given index.
func make_choice(index: int) -> void:
	if _engine != null:
		_engine.MakeChoice(index)

# Effect text from the most recent make_choice() call.
func get_last_effect_text() -> String:
	return _engine.LastEffectText ?? "" if _engine != null else ""

# ── History ────────────────────────────────────────────────────────────────────

func get_history_length() -> int:
	return _engine.HistoryLength if _engine != null else 0

func get_history_entry(index: int) -> Dictionary:
	if _engine == null:
		return {}
	var entry = _engine.GetHistoryEntry(index)
	if entry == null:
		return {}
	return {
		"event_id": entry.EventId,
		"choice_index": entry.ChoiceIndex,
		"choice_text": entry.ChoiceText
	}

func get_all_history() -> Array:
	if _engine == null:
		return []
	var entries = _engine.GetAllHistory()
	var result = []
	for entry in entries:
		result.append({
			"event_id": entry.EventId,
			"choice_index": entry.ChoiceIndex,
			"choice_text": entry.ChoiceText
		})
	return result

# ── State ────────────────────────────────────────────────────────────────────

# Serialize the full player state as JSON (for save files).
func get_state_json() -> String:
	return _engine.GetStateJson() ?? "" if _engine != null else ""

# Restore state from a JSON string.
func set_state_json(json: String) -> void:
	if _engine != null:
		_engine.SetStateJson(json)

# ── Queries ────────────────────────────────────────────────────────────────────

func get_available_events() -> PackedStringArray:
	if _engine == null:
		return []
	return PackedStringArray(_engine.GetAvailableEvents())

func can_access_event(id: String) -> bool:
	return _engine.CanAccessEvent(id) if _engine != null else false

# ── Stats / tags / flags ─────────────────────────────────────────────────────

func get_stats() -> Dictionary:
	if _engine == null:
		return {}
	return _engine.GetStats()

func get_stat(name: String) -> int:
	return _engine.GetStat(name) if _engine != null else 0

func get_story_tags() -> PackedStringArray:
	if _engine == null:
		return []
	return PackedStringArray(_engine.GetStoryTags())

func get_tags() -> PackedStringArray:
	if _engine == null:
		return []
	return PackedStringArray(_engine.GetTags())

func get_flags() -> PackedStringArray:
	if _engine == null:
		return []
	return PackedStringArray(_engine.GetFlags())

# ── Cleanup ──────────────────────────────────────────────────────────────────

func _exit_tree():
	if _engine != null and _engine.has_method("Dispose"):
		_engine.Dispose()
	if _catalog != null and _catalog.has_method("Dispose"):
		_catalog.Dispose()
	_engine = null
	_catalog = null
