# CYOA Language Syntax Reference

> Extended reference for the `.cyoa` DSL. For the canonical grammar, see
> [SPEC.md](../SPEC.md). This document provides detailed examples and
> advanced patterns.

## Table of Contents

- [File Structure](#file-structure)
- [Story Declaration](#story-declaration)
- [Stats, Flags, and Tags](#stats-flags-and-tags)
- [Imports](#imports)
- [Effects](#effects)
- [Events](#events)
- [Choices](#choices)
- [Prerequisites (Conditions)](#prerequisites-conditions)
- [Text Templating](#text-templating)
- [Standard Library](#standard-library)
- [Complete Example](#complete-example)

---

## File Structure

A `.cyoa` file has exactly one `story` block at the top level:

```cyoa
story StoryName:

  # all declarations live inside the story block, indented 2 spaces
  stat hp = 50

  event start:
    "You begin your journey."
```

- **Indentation**: 2 spaces (tabs are rejected).
- **Comments**: Start with `#` and extend to end of line.
- **Quotes**: Text strings may use double or single quotes.

---

## Story Declaration

Every `.cyoa` file starts with a `story` keyword followed by the story name
and a colon:

```cyoa
story ForestAdventure:
  # story body — everything is indented by 2 spaces
```

- Only one `story` block per file.
- The story name is used as an identifier by the API (`createEngineByName`).
- Story-level `tags:` appears immediately after the story declaration.

---

## Stats, Flags, and Tags

### Stats (Numeric)

Stats are `i64` values — wide range for game numbers (health, gold, currency, etc.).

```cyoa
stat hp = 50          # initial value
stat courage = 0
stat gold = 0        # defaults to 0
```

- Scope: Global to the story.
- All stats in all events can read/write any stat.
- Stats can appear anywhere in the story block (before or after events).

### Flags (Boolean)

Flags are boolean markers for story progress.

```cyoa
flag visited_cave        # defaults to false
flag obtained_key = false # explicit false
flag mushroom_collected = true  # starts true
```

- Use `set flag to true` or `set flag to false` in effects/choices to change.
- Use bare `flag_name` in `requires:` to check.

### Tags

Tags exist at two levels:

#### Story-level Tags

Declared once at the top of the story block. These are static metadata
used to filter stories via the `StoryCatalog` API.

```cyoa
story ForestAdventure:

  tags: fantasy, exploration

  stat hp = 50
  ...
```

**Multi-line form** — when `tags:` is on its own line, tag names follow on
indented lines (one per line or comma-separated per line):

```cyoa
story ForestAdventure:

  tags:
    fantasy
    exploration

  stat hp = 50
  ...
```

#### Event-level Tags

Declared per-event, describing that event's content.

```cyoa
event old_ruins:
  tags: exploration, early_game
```

**Multi-line form:**

```cyoa
event old_ruins:
  tags:
    exploration
    early_game
```

> **Note**: Story-level tags are distinct from runtime tags (see `add tag`
> in effects). Story tags are static; runtime tags are applied dynamically
> during play and are part of `PlayerState`.

---

## Imports

Writers can import definitions from other `.cyoa` files. All imports are
resolved and merged into a single bytecode file at compile time.

### Syntax

```cyoa
import "std/healing"        # standard library package
import "./my_stats"          # relative to current file
import "./effects" as eff    # aliased to avoid name collisions
```

### Standard Library

Shipped with the compiler. Imported via `import "std/..."`:

| Package | Effects |
|---------|---------|
| `std/healing` | `healing_potion` (+20 HP), `minor_heal` (+10 HP), `full_restore` (+999999 HP) |
| `std/combat` | `basic_attack` (+1 courage), `critical_hit` (+5 courage), `dodge` (+2 courage) |

### How Merge Works

1. **Parse pass**: The compiler parses the main file and all transitively
   imported files into ASTs.
2. **Merge pass**: All ASTs are combined into one unified tree. Name
   collisions are detected and reported as errors.
3. **Codegen pass**: The merged AST is compiled into a single `.cyoa.bc`.

### Circular Import Detection

Circular imports are a compile error:

```
Error: circular import detected: A → B → A
```

---

## Effects

Effects are reusable consequence blocks: define once, reference from
multiple choices using `uses`.

```cyoa
effect found_mushroom:
  + courage by 1
  text "You find a glowing mushroom. It hums softly."
```

### Effect Body Syntax

Inside an effect block (indented 4 spaces), you can use:

| Syntax | Description |
|--------|-------------|
| `+ stat by N` | Increase stat by N |
| `- stat by N` | Decrease stat by N |
| `set flag to true` | Set flag to true |
| `set flag to false` | Set flag to false |
| `add tag` | Add a runtime tag |
| `text "string"` | Text output to display |
| `text "string with {{templating}}"` | Templated text output |

You can also use unquoted text as shorthand for `text`:

```cyoa
effect found_mushroom:
  + courage by 1
  "You find a glowing mushroom. It hums softly."  # equivalent to: text "..."
```

### Referencing Effects

Use `uses` in a choice to append an effect block:

```cyoa
choice "Follow the trail":
  uses found_mushroom
  next deep_woods
```

You can use multiple effects (comma-separated):

```cyoa
choice "Both options":
  uses found_mushroom, wolf_scare
  next somewhere
```

---

## Events

Events are the core story nodes. Each event has optional prerequisites,
tags, text, and choices.

```cyoa
event old_ruins:
  requires: courage >= 5           # optional prerequisite
  tags: exploration, early_game     # optional event tags

  set visited_old_ruins to true    # optional inline effect (runs on entry)
  "You stand before ancient stone ruins."
  "A cold wind whispers from within."

  choice "Enter the ruins":
    next river_crossing

  choice "Circle around":
    uses found_mushroom
    next dense_forest
```

### Event Fields

| Field | Required? | Description |
|-------|-----------|-------------|
| `requires:` | No | AND/OR condition expression (inline or multi-line) |
| `tags:` | No | Comma-separated tag list (inline or multi-line) |
| Inline effects | No | `set`, `+/- stat`, `add tag` that run when event is entered |
| Text lines | No | Quoted or unquoted prose shown to the player |
| `choice:` | Yes | At least one choice (or the event is terminal) |

### Event Entry Effects

Any effect instructions placed directly in the event body (not inside a
choice) run automatically when the engine enters the event:

```cyoa
event old_ruins:
  set visited_old_ruins to true    # runs once on entry
  "You stand before ancient stone ruins."
```

This is useful for setting progress flags when the player reaches a location,
regardless of which choice they made to get there.

---

## Choices

Choices are the player-selectable options. Each choice can have inline
effects, reference reusable effects, and specify the next event.

```cyoa
choice "Attack the wolf":
  + courage by 2                    # inline stat change
  uses wolf_scare                   # append reusable effect
  "You charge at the wolf."         # text shown after choice
  next wolf_fight                   # advance to next event
```

### Choice Fields

| Field | Required? | Description |
|-------|-----------|-------------|
| Text | Yes | The choice label shown to the player |
| Inline effects | No | `+/- stat`, `set flag`, `text`, `add tag` |
| `uses` | No | Reference to one or more effect blocks |
| `requires:` | No | Local prerequisite (filter this choice) |
| `next` | No* | Event to advance to (`*required` for non-terminal choices) |

### Choice Text Can Be Templated

```cyoa
choice "Buy ale (cost: {{gold}} gold)":
  requires: gold >= 5
  - gold by 5
  "You buy an ale. The barkeep grins."
  next tavern
```

### Terminal Choices

A choice without `next` ends the story:

```cyoa
choice "Rest forever":
  "You sit beneath the tree until the sun sets."
  # no `next` — story ends
```

---

## Prerequisites (Conditions)

The `requires:` field supports AND/OR logic and stat/flag comparisons.

### Operators

```cyoa
requires: courage >= 5                    # stat threshold
requires: hp > 0                          # greater than
requires: gold >= 5 AND reputation >= 10  # AND
requires: gold >= 5 OR reputation >= 10    # OR
requires: NOT defeated_dragon             # negation of flag
requires: (courage >= 5 OR gold >= 100) AND visited_old_ruins  # grouping
```

### Multi-line Form

When the condition is long, `requires:` can stand alone on its line with the
condition on the following indented lines:

```cyoa
event guarded_path:
  requires:
    courage >= 5 AND gold > 0
  "A guarded path lies ahead."
```

### Full Operator Table

| Operator | Meaning |
|----------|---------|
| `>=` | Greater than or equal (stats only) |
| `<=` | Less than or equal (stats only) |
| `>` | Greater than (stats only) |
| `<` | Less than (stats only) |
| `==` | Equal to (stats only) |
| `!=` | Not equal to (stats only) |
| `AND` | Logical AND |
| `OR` | Logical OR |
| `NOT` | Logical NOT (prefix) |
| `( )` | Grouping (parentheses) |
| bare identifier | True if flag is set |

> **Note**: Comparisons work only with stats (numeric). For flags (boolean),
> use the bare flag name — it evaluates to true if the flag is currently set.

---

## Text Templating

Writers can interpolate current stat values into prose text using `{{var}}`
syntax (double curly braces).

### Basic Templating

```cyoa
event tavern:
  "You have {{gold}} gold pieces to spend."
```

At runtime, `{{gold}}` is replaced with the current value of the `gold` stat.

### Templated Choices

```cyoa
choice "Buy ale (cost: {{gold}} gold)":
  requires: gold >= 5
```

### Multiple Templates in One String

```cyoa
"You have {{gold}} gold pieces and {{hp}} HP remaining."
```

### Template Rules

- **Only stats are interpolated** (numeric `i64`). Flags (boolean) are
  not interpolated.
- **Missing variables**: If a variable hasn't been declared, the placeholder
  renders literally (e.g., `{{gold}}` stays as-is) — so writers notice
  the missing declaration.
- **Compile-time parsing**: At compile time, template text is parsed into
  segments: literal text + stat lookup. At runtime, the VM renders
  templates atomically using the `RenderTemplate` instruction.
- **Game engines always receive pre-rendered strings** — no template
  syntax in the API output.

---

## Standard Library

### `std/healing`

```cyoa
import "std/healing"

# Available effects:
# healing_potion: +20 hp, "You drink a healing potion. You recover 20 HP."
# minor_heal:    +10 hp, "You take a moment to rest. You recover 10 HP."
# full_restore:  +999999 hp, "You feel completely revitalized."
```

### `std/combat`

```cyoa
import "std/combat"

# Available effects:
# basic_attack:  +1 courage, "You strike at your opponent."
# critical_hit:  +5 courage, "A critical hit! Your opponent reels."
# dodge:         +2 courage, "You gracefully dodge the attack."
```

---

## Complete Example

```cyoa
# Example: A story using all major features
import "std/healing"
import "std/combat"

story MyAdventure:

  tags: fantasy, combat

  stat hp = 50
  stat courage = 0
  stat gold = 0
  flag visited_cave
  flag has_key

  effect found_treasure:
    + gold by 20
    text "You discover a cache of gold coins!"
    add treasure_hunter

  event start:
    tags: early_game
    "You stand at a crossroads."
    "Three paths stretch before you."

    choice "Take the forest path":
      next forest_path

    choice "Enter the mountain cave":
      requires: courage >= 2
      set visited_cave to true
      next mountain_cave

    choice "Go home":
      - gold by 0
      # terminal choice (no `next`)

  event forest_path:
    "The forest is dark and quiet."
    "You have {{hp}} HP."

    choice "Press deeper":
      uses basic_attack
      next deep_forest

    choice "Return":
      next start

  event mountain_cave:
    tags: combat, dangerous
    "A bat swarm attacks!"
    "You have {{gold}} gold."

    choice "Fight with sword":
      uses critical_hit
      uses healing_potion
      next victory

    choice "Use healing potion":
      requires: gold >= 5
      uses healing_potion
      - gold by 5
      next start

  event victory:
    tags: completion
    "You emerge victorious!"
    "Final: hp={{hp}}, courage={{courage}}, gold={{gold}}"

    choice "Play again":
      next start
```