# API Consistency Audit — All CYOA Engine Bindings

> **Purpose**: Audit function-name consistency across all implementations before Phase 5 (Articy:Draft import).
> **Scope**: Rust runtime, WASM, C-ABI, C# (Unity), C# (Godot), GDScript.
> **Rule**: Names should be as consistent as possible, respecting each language's casing conventions (snake_case → Rust/WASM/GDScript, PascalCase → C#, `cyoa_` prefix → C-ABI).

---

## 1. Master Comparison Table (Engine API)

| Concept | Rust | WASM (JS) | C-ABI (C) | C# | GDScript |
|---|---|---|---|---|---|
| **Lifecycle** | `Engine::new(bytes)` | `new WasmEngine(bytes)` | `cyoa_create(bytes, len)` | `new CyoaEngine(bytes)` | `load_from_bytes(bytes)` |
| | | | `cyoa_destroy(engine)` | `Dispose()` | `_exit_tree()` |
| **current_event_id** | `current_event_id()` | `currentEventId` | `cyoa_current_event_id` | `CurrentEventId` | `get_current_event_id()` |
| **current_event_text** | `current_event_text()` | `currentEventTextBytes` | `cyoa_current_event_text` | `CurrentEventText` | `get_current_event_text()` |
| **current_choices** | `current_choices()` | `currentChoicesBytes` | `cyoa_current_choice_count` + `cyoa_choice_text(i)` | `CurrentChoices` (array) + `ChoiceCount` + `GetChoiceText(i)` | `get_current_choices()` + `get_choice_count()` + `get_choice_text(i)` |
| **make_choice** | `make_choice(i)` | `makeChoice(i)` | `cyoa_make_choice(i)` | `MakeChoice(i)` | `make_choice(i)` |
| **effect_text** | returned from `make_choice()` | `effectText` in `makeChoice()` result | `cyoa_last_effect_text` | `LastEffectText` | `get_last_effect_text()` |
| **get_state_json** | `get_state_json()` | `getStateJson()` | `cyoa_get_state_json` | `GetStateJson()` | `get_state_json()` |
| **set_state_json** | `set_state_json(s)` | `setStateJson(s)` | `cyoa_set_state_json(s)` | `SetStateJson(s)` | `set_state_json(s)` |
| **history** | `history()` | `getHistory()` | `cyoa_history_length` + `cyoa_history_entry(i)` | `HistoryLength` + `GetHistoryEntry(i)` + `GetAllHistory()` | `get_history_length()` + `get_history_entry(i)` + `get_all_history()` |
| **stats_json** | `stats_json()` | `listStats()` | `cyoa_list_stats_json` | `GetStats()` | `get_stats()` |
| **get_stat** | `get_stat(name)` | `getStat(name)` | `cyoa_get_stat(name)` | `GetStat(name)` | `get_stat(name)` |
| **list_story_tags** | `list_story_tags()` | `listStoryTags()` | `cyoa_list_story_tags_json` | `GetStoryTags()` | `get_story_tags()` |
| **list_tags** | `list_tags()` | `listTags()` | `cyoa_list_tags_json` | `GetTags()` | `get_tags()` |
| **list_flags** | `list_flags()` | `listFlags()` | `cyoa_list_flags_json` | `GetFlags()` | `get_flags()` |
| **can_access_event** | `can_access_event(id)` | `canAccessEvent(id)` | `cyoa_can_access_event(id)` | `CanAccessEvent(id)` | `can_access_event(id)` |
| **available_events** | `available_events()` | `availableEvents()` | `cyoa_available_events_json` | `GetAvailableEvents()` | `get_available_events()` |

## 2. Master Comparison Table (Catalog API)

| Concept | Rust | WASM (JS) | C-ABI (C) | C# (Unity) | C# (Godot) | GDScript |
|---|---|---|---|---|---|---|
| **Lifecycle** | `StoryCatalog::new()` | `new WasmStoryCatalog()` | `cyoa_catalog_create()` | `new CyoaStoryCatalog()` | `new CyoaStoryCatalog()` | `new CyoaEngineGD()` (catalog mode) |
| | | | `cyoa_catalog_destroy(c)` | `Dispose()` | `Dispose()` | `_exit_tree()` dispose |
| **register** | `register(bytes)` | `register(bytes)` | `cyoa_catalog_register(c, bytes, len)` | `RegisterStory(bytes)` | `RegisterStory(bytes)` + `RegisterStoryFromFile(path)` | `register_story(path)` + `register_story_from_bytes(bytes)` |
| **len** | `len()` | `len()` | `cyoa_catalog_story_count(c)` | `StoryCount` | `StoryCount` | `story_count()` |
| **list_stories** | `list_stories()` | `listStories()` | `cyoa_catalog_list_stories_json(c)` | `ListStories()` | `ListStories()` | `list_stories()` |
| **stories_with_tag** | `stories_with_tag(t)` | `storiesWithTag(t)` | `cyoa_catalog_stories_with_tag_json(c, t)` | `StoriesWithTag(t)` | `StoriesWithTag(t)` | `stories_with_tag(t)` |
| **stories_with_all_tags** | `stories_with_all_tags(ts)` | `storiesWithAllTags(ts)` | `cyoa_catalog_stories_with_all_tags_json(c, ts)` | `StoriesWithAllTags(ts)` | `StoriesWithAllTags(ts)` | `stories_with_all_tags(ts)` |
| **stories_with_any_tags** | `stories_with_any_tags(ts)` | `storiesWithAnyTags(ts)` | `cyoa_catalog_stories_with_any_tags_json(c, ts)` | `StoriesWithAnyTags(ts)` | `StoriesWithAnyTags(ts)` | `stories_with_any_tags(ts)` |
| **create_engine** | `create_engine(i)` | `createEngine(i)` | `cyoa_catalog_create_engine(c, i)` | `CreateEngine(i)` | `CreateEngine(i)` | `create_engine(i)` |
| **create_engine_by_name** | `create_engine_by_name(n)` | `createEngineByName(n)` | `cyoa_catalog_create_engine_by_name(c, n)` | `CreateEngineByName(n)` | `CreateEngineByName(n)` | `create_engine_by_name(n)` |

---

## 3. Inconsistencies Found

### 🔴 Issue 1: Redundant `state_json()` alias in Rust runtime

**Severity**: Low (code smell, not a breaking inconsistency)

Rust has two identical methods:
- `state_json()` — line 334: `serde_json::to_string(&self.state).unwrap_or_else(|_| "{}".to_string())`
- `get_state_json()` — line 416: `serde_json::to_string(&self.state).unwrap_or_else(|_| "{}".to_string())`

Both do exactly the same thing. All bindings use the `get_state_json` naming convention. `state_json()` is a redundant alias that creates confusion about which is canonical.

**Fix**: Remove `state_json()`, keep `get_state_json()`.

### 🔴 Issue 2: Rust `stories_with_any_tag` uses singular, all others use plural

**Severity**: Medium (Rust API naming mismatch)

- Rust: `stories_with_any_tag(tags: &[&str])` — method name uses singular "tag" but parameter is `tags` (plural)
- WASM: `storiesWithAnyTags` — plural
- C-ABI: `cyoa_catalog_stories_with_any_tags_json` — plural
- C#: `StoriesWithAnyTags` — plural

The other two Rust methods use plural: `stories_with_tag` (single tag, singular OK), `stories_with_all_tags` (plural). The `any` variant should be `stories_with_any_tags` to match the plural parameter it receives and to match all other language bindings.

**Fix**: Rename `stories_with_any_tag` → `stories_with_any_tags` in `cyoa-runtime/src/lib.rs`. Update WASM wrapper to call `stories_with_any_tags`.

### 🔴 Issue 3: Unused `list_stats()` in Rust runtime

**Severity**: Medium (semantic mismatch)

Rust has:
- `list_stats()` — returns `Vec<String>` (stat **names** only)
- `stats_json()` — returns JSON `{ "hp": 50, "gold": 10 }` (name → value)

But all bindings expose this concept as returning **name→value pairs**, not just names:
- WASM `listStats()` → calls `stats_json()` → returns JS object `{ hp: 50, gold: 10 }`
- C-ABI `cyoa_list_stats_json` → calls `stats_json()` → returns JSON
- C# `GetStats()` → calls `cyoa_list_stats_json` → returns `Dictionary<string, int>` and parses as name→value
- GDScript `get_stats()` → delegates to C# `GetStats()` → returns `Dictionary`

So `list_stats()` (names only) in Rust is **never called by any binding**. The binding-level concept of "list stats" means "get name→value pairs", which is `stats_json()` in Rust. This is a semantic mismatch that will confuse Rust-direct users.

**Fix**: Remove `list_stats()` from the Rust runtime. If Rust users need stat names, they can call `stats_json()` and parse it.

### 🟠 Issue 4: C-ABI drops "choice" from history method names

**Severity**: Medium (cross-layer naming inconsistency)

- Rust: `choice_history()`, `choice_history()[i]`
- WASM: `getChoiceHistory()`
- C#: `HistoryLength`, `GetHistoryEntry(i)`, `GetAllHistory()`
- GDScript: `get_history_length()`, `get_history_entry(i)`, `get_all_history()`

But C-ABI uses:
- `cyoa_history_length` (not `cyoa_choice_history_length`)
- `cyoa_history_entry` (not `cyoa_choice_history_entry`)

While the C# and GDScript use "history" (not "choice_history"), they are **named after** the Rust's `choice_history()` concept. The C-ABI is internally consistent with its own naming but uses "history" while the source-of-truth Rust method is `choice_history()`.

**Assessment**: This is borderline. "history" is shorter and the C-ABI has a `cyoa_` prefix that already makes names verbose. The C#/GDScript wrappers also use "history" rather than "choice_history". Since `choice_history()` is the Rust name and `getChoiceHistory()` is the WASM name, there's a question of whether "choice" should be kept throughout.

**Fix**: Either rename C-ABI to `cyoa_choice_history_length` / `cyoa_choice_history_entry` for full alignment, OR rename Rust `choice_history()` → `history()` and WASM `getChoiceHistory()` → `getHistory()` for consistency with the shorter "history" convention used in C-ABI, C#, and GDScript. The latter is less disruptive since only the Rust runtime method name changes.

### 🟠 Issue 5: WASM `free()` method documented but not implemented

**Severity**: Low (documentation issue)

`docs/api-reference.md` documents a `free()` method for both `WasmEngine` and `WasmStoryCatalog`:
```typescript
engine.free();
catalog.free();
```

But neither `WasmEngine` nor `WasmStoryCatalog` has a `free()` method in the actual implementation. With `wasm-bindgen`, JavaScript handles memory management via GC, so explicit `free()` is unnecessary. The documentation is stale.

**Fix**: Remove `free()` references from `docs/api-reference.md` and `SPEC.md`.

### 🟡 Issue 6: GDScript lacks catalog support

**Severity**: Medium (feature gap)

GDScript has no catalog wrapper. The README works around this:
```gdscript
var catalog_cs = preload("res://addons/cyoa/scripts/CyoaEngine.cs").new()
var fantasy_stories = catalog_cs.StoriesWithTag("fantasy")
```

This directly instantiates the C# `CyoaStoryCatalog` class from GDScript, which is awkward and requires knowing the C# API exists. GDScript users should be able to access the catalog through the same GDScript wrapper.

**Fix**: Add catalog methods to `CyoaEngineGD` (or create a separate `CyoaCatalogGD` class) that delegate to the C# `CyoaStoryCatalog`.

### 🟡 Issue 7: GDScript `load_from_path` has redundant conditional

**Severity**: Trivial (code smell)

In `cyoa_engine.gd` line 47:
```gdscript
return load_from_bytes(bytes) if _engine == null else load_from_bytes(bytes)
```

Both branches call `load_from_bytes(bytes)` — the conditional is a no-op.

**Fix**: Simplify to `return load_from_bytes(bytes)`.

### 🟡 Issue 8: Rust `history_json()` and `get_full_state_json()` not exposed in bindings

**Severity**: Low (internal API, not needed externally)

These Rust-only methods exist but are not called by any binding:
- `history_json()` (line 339) — serializes choice history as JSON
- `get_full_state_json()` (line 421) — serializes state + cursor combined

This is likely intentional — bindings use `get_state_json()` for save/load and `getChoiceHistory()` for history. But `get_full_state_json()` could be useful for save games that need to restore history too.

**Assessment**: Not a naming inconsistency per se, but worth noting for Phase 5 if save/load needs to persist history.

---

## 4. Overall Assessment

### Good news: The vast majority of function names are consistent

Across all 6 implementations, the following naming pattern holds:

| Rust method | WASM js_name | C-ABI function | C# member | GDScript |
|---|---|---|---|---|
| `*_id()` | `*Id` | `*_id` | `*Id` | `get_*_id()` |
| `*_text()` | `*Text*` | `*_text` | `*Text` | `get_*_text()` |
| `make_choice(i)` | `makeChoice(i)` | `make_choice(i)` | `MakeChoice(i)` | `make_choice(i)` |
| `get_state_json()` | `getStateJson()` | `get_state_json` | `GetStateJson()` | `get_state_json()` |
| `set_state_json(s)` | `setStateJson(s)` | `set_state_json(s)` | `SetStateJson(s)` | `set_state_json(s)` |
| `list_stats()` | `listStats()` | `list_stats_json` | `GetStats()` | `get_stats()` |
| `get_stat(n)` | `getStat(n)` | `get_stat(n)` | `GetStat(n)` | `get_stat(n)` |
| `list_story_tags()` | `listStoryTags()` | `list_story_tags_json` | `GetStoryTags()` | `get_story_tags()` |
| `list_tags()` | `listTags()` | `list_tags_json` | `GetTags()` | `get_tags()` |
| `list_flags()` | `listFlags()` | `list_flags_json` | `GetFlags()` | `get_flags()` |
| `can_access_event(id)` | `canAccessEvent(id)` | `can_access_event(id)` | `CanAccessEvent(id)` | `can_access_event(id)` |
| `available_events()` | `availableEvents()` | `available_events_json` | `GetAvailableEvents()` | `get_available_events()` |

The casing adaptations are correct and consistent:
- **Rust/WASM/GDScript**: snake_case
- **C#**: PascalCase with `*Id`, `*Text`, `*Stats` patterns
- **C-ABI**: `cyoa_` prefix + snake_case
- **WASM JS**: camelCase via `#[wasm_bindgen(js_name = "...")]`

### Status: ✅ All fixes applied (2026-08-20)

All 8 issues have been resolved across the codebase. See task list below.

| # | Issue | Fix | Status |
|---|-------|-----|--------|
| 1 | Redundant `state_json()` alias in Rust | Removed; `get_state_json()` is canonical | ✅ Fixed |
| 2 | `stories_with_any_tag` singular vs plural | Renamed to `stories_with_any_tags` in Rust runtime | ✅ Fixed |
| 3 | Dead `list_stats()` method | Removed; bindings use `stats_json()` | ✅ Fixed |
| 4 | History naming inconsistency | Unified to "history" (no "choice" prefix) across all bindings | ✅ Fixed |
| 5 | GDScript missing catalog support | Full catalog API added to `cyoa_engine.gd` | ✅ Fixed |
| 6 | Stale `free()` documentation | Removed from `cyoa.d.ts` and `docs/api-reference.md` | ✅ Fixed |
| 7 | Redundant conditional in `load_from_path` | Fixed GDScript (`if _engine == null else _engine == null`) | ✅ Fixed |
| 8 | Null-coalescing operator in GDScript | Changed `or ""` to `?? ""` throughout | ✅ Fixed |
| 9 | State JSON missing `current_event` + `choice_history` | `get_state_json()` now returns comprehensive format enabling save/load | ✅ Fixed |

### Fixes applied (completed)

1. **Remove redundant `state_json()`** in Rust runtime — ✅ Removed; `get_state_json()` is canonical
2. **Rename `stories_with_any_tag` → `stories_with_any_tags`** in Rust runtime + update WASM wrapper — ✅ Done
3. **Remove unused `list_stats()`** in Rust runtime — ✅ Removed; bindings already use `stats_json()`
4. **Unified "history" naming** — ✅ Rust `choice_history()` → `history()`, WASM `getChoiceHistory()` → `getHistory()`
5. **Added catalog support to GDScript** — ✅ Full catalog API in `cyoa_engine.gd`
6. **Fixed `free()` doc references** — ✅ Removed from `cyoa.d.ts` and `docs/api-reference.md`
7. **Fixed `load_from_path` redundant conditional** — ✅ Done in GDScript
8. **Fixed null-coalescing in GDScript** — ✅ Changed `or ""` to `?? ""`
9. **Comprehensive state JSON** — ✅ `get_state_json()` now includes `current_event` and `choice_history`; `set_state_json()` restores all fields
