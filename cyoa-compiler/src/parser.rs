//! Line-based parser for the CYOA DSL.
//!
//! The DSL is indentation-based (2-space indent per level). This parser
//! reads lines, tracks indentation, and builds a `Story` AST.
//!
//! Grammar reference: see `grammar.pest` for the official pest grammar.
//! This hand-written parser implements equivalent logic with better
//! writer-friendly error messages.

use cyoa_ast::*;

/// Error from parsing.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl ParseError {
    fn at(msg: impl Into<String>, line: usize, col: usize) -> Self {
        Self {
            message: msg.into(),
            line,
            col,
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parse a `.cyoa` source string into a `Story` AST.
pub fn parse_story(input: &str) -> Result<Story, ParseError> {
    let lines = tokenize_lines(input);
    let mut cursor = LineCursor::new(lines);
    let story = parse_story_from_lines(&mut cursor)?;

    if cursor.has_more() {
        let line = cursor.peek();
        return Err(ParseError::at(
            format!("unexpected content after story block: '{}'", line.content),
            line.line_num,
            line.col,
        ));
    }
    Ok(story)
}

// ===== Line tokenization =====

#[derive(Debug, Clone)]
pub struct Line {
    pub indent: usize,
    pub content: String,
    pub line_num: usize,
    pub col: usize,
}

/// Split input into lines, counting indentation and stripping comments.
fn tokenize_lines(input: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    for (i, raw) in input.lines().enumerate() {
        let (indent, raw_content) = strip_indent(raw);
        let stripped = strip_comment(raw_content);
        // Trim trailing whitespace but preserve content
        let content = stripped.trim_end().to_string();
        lines.push(Line {
            indent,
            content,
            line_num: i + 1,
            col: indent + 1,
        });
    }
    lines
}

/// Count leading spaces for indentation.
fn strip_indent(line: &str) -> (usize, &str) {
    let leading = line.chars().take_while(|c| *c == ' ').count();
    (leading, &line[leading..])
}

/// Strip a `#` comment from a line, respecting quoted strings.
fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escape = false;
    for (i, c) in line.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            in_string = true;
        } else if c == '#' {
            return &line[..i];
        }
    }
    line
}

// ===== Line cursor =====

struct LineCursor {
    lines: Vec<Line>,
    pos: usize,
}

impl LineCursor {
    fn new(lines: Vec<Line>) -> Self {
        Self { lines, pos: 0 }
    }

    fn has_more(&self) -> bool {
        self.pos < self.lines.len()
    }

    fn peek(&self) -> &Line {
        self.lines
            .get(self.pos)
            .expect("LineCursor::peek with no more lines")
    }

    fn next(&mut self) -> Option<Line> {
        let line = self.lines.get(self.pos).cloned();
        if line.is_some() {
            self.pos += 1;
        }
        line
    }

    fn current_indent(&self) -> usize {
        self.lines.get(self.pos).map(|l| l.indent).unwrap_or(0)
    }
}

// ===== Story parser =====

fn parse_story_from_lines(cursor: &mut LineCursor) -> Result<Story, ParseError> {
    let mut imports = Vec::new();
    let mut items = Vec::new();
    let mut story_tags = Vec::new();
    let mut story_name = String::new();
    let mut found_story = false;

    while cursor.has_more() {
        let line = cursor.peek().clone();
        if line.content.trim().is_empty() {
            cursor.next();
            continue;
        }

        let keyword = line.content.split_whitespace().next().unwrap_or("");
        match keyword {
            "import" => {
                cursor.next();
                let import = parse_import(&line.content, line.line_num, line.col)?;
                imports.push(import);
            }
            "story" => {
                cursor.next();
                story_name = parse_story_header(&line.content, line.line_num, line.col)?;
                found_story = true;

                // Skip blank lines to find base indent
                while cursor.has_more() {
                    let peeked = cursor.peek();
                    if peeked.content.trim().is_empty() {
                        cursor.next();
                    } else {
                        break;
                    }
                }
                let base_indent = cursor.current_indent();
                parse_story_body(cursor, &mut items, &mut story_tags, base_indent)?;
            }
            // Library files (imports) may define items at the file scope (indent 0)
            // without a `story` wrapper. Handle stat/flag/effect/event at top level.
            "stat" => {
                cursor.next();
                let item =
                    StoryItem::StatDef(parse_stat_def(&line.content, line.line_num, line.col)?);
                items.push(item);
            }
            "flag" => {
                cursor.next();
                let item =
                    StoryItem::FlagDef(parse_flag_def(&line.content, line.line_num, line.col)?);
                items.push(item);
            }
            "effect" => {
                cursor.next();
                let (eff, _) = parse_effect_block(cursor, line.line_num, line.col, &line.content)?;
                items.push(StoryItem::EffectDef(eff));
            }
            "event" => {
                cursor.next();
                let (ev, _) = parse_event_block(cursor, line.line_num, line.col, &line.content)?;
                items.push(StoryItem::EventDef(ev));
            }
            _ => {
                return Err(ParseError::at(
                    format!(
                        "unexpected keyword '{}' at file scope, expected 'import', 'story', 'stat', 'flag', 'effect', or 'event'",
                        keyword
                    ),
                    line.line_num,
                    line.col,
                ));
            }
        }
    }

    // Library files (no story block) are valid — they just contain definitions
    if !found_story && items.is_empty() {
        return Err(ParseError::at(
            "no story block or items found; expected 'story StoryName:' or declarations",
            0,
            0,
        ));
    }

    Ok(Story {
        name: story_name,
        imports,
        tags: story_tags,
        items,
    })
}

fn parse_import(content: &str, line: usize, col: usize) -> Result<Import, ParseError> {
    let rest = content.strip_prefix("import").unwrap().trim();

    let (path_str, alias) = if let Some(pos) = rest.rfind(" as ") {
        let path_part = &rest[..pos];
        let alias_str = &rest[pos + 4..];
        (path_part, Some(alias_str.to_string()))
    } else {
        (rest, None)
    };

    let path = parse_string_literal(path_str, line, col)?;
    Ok(Import { path, alias })
}

fn parse_story_header(content: &str, line: usize, col: usize) -> Result<String, ParseError> {
    let rest = content.strip_prefix("story").unwrap();
    let rest = rest.trim();
    let rest = rest.trim_end_matches(':').trim();
    if rest.is_empty() {
        return Err(ParseError::at(
            "story name is missing after 'story:'",
            line,
            col,
        ));
    }
    Ok(rest.to_string())
}

/// Parse all items inside a story block.
fn parse_story_body(
    cursor: &mut LineCursor,
    items: &mut Vec<StoryItem>,
    story_tags: &mut Vec<String>,
    base_indent: usize,
) -> Result<(), ParseError> {
    loop {
        if !cursor.has_more() {
            break;
        }

        let line = cursor.peek().clone();

        // Skip blank lines
        if line.content.trim().is_empty() {
            cursor.next();
            continue;
        }

        // If indent is less than base, we're done with this block
        if line.indent < base_indent {
            break;
        }

        // If indent is greater than base, it's an unexpected sub-block
        if line.indent > base_indent {
            return Err(ParseError::at(
                format!("unexpected indentation: '{}'", line.content),
                line.line_num,
                line.col,
            ));
        }

        // Handle story-level tags (not a StoryItem)
        if line.content.starts_with("tags:") {
            cursor.next();
            let rest = line.content.strip_prefix("tags:").unwrap().trim();
            story_tags.extend(
                rest.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
            );
            continue;
        }

        // At the correct indent level — parse the item
        let keyword = line.content.split_whitespace().next().unwrap_or("");
        let item = match keyword {
            "import" => {
                cursor.next();
                let import = parse_import(&line.content, line.line_num, line.col)?;
                StoryItem::Import(import)
            }
            "stat" => {
                cursor.next();
                StoryItem::StatDef(parse_stat_def(&line.content, line.line_num, line.col)?)
            }
            "flag" => {
                cursor.next();
                StoryItem::FlagDef(parse_flag_def(&line.content, line.line_num, line.col)?)
            }
            "effect" => {
                cursor.next(); // Consume the header line
                let (eff, _) = parse_effect_block(cursor, line.line_num, line.col, &line.content)?;
                StoryItem::EffectDef(eff)
            }
            "event" => {
                cursor.next(); // Consume the header line
                let (ev, _) = parse_event_block(cursor, line.line_num, line.col, &line.content)?;
                StoryItem::EventDef(ev)
            }
            _ => {
                // Possible comment (but we already stripped comments)
                if line.content.starts_with('#') {
                    cursor.next();
                    continue;
                }
                return Err(ParseError::at(
                    format!("unexpected line in story body: '{}'", line.content),
                    line.line_num,
                    line.col,
                ));
            }
        };

        items.push(item);
    }
    Ok(())
}

fn parse_stat_def(content: &str, line: usize, col: usize) -> Result<StatDef, ParseError> {
    let rest = content.strip_prefix("stat").unwrap().trim();
    let (name, value_str) = rest
        .split_once('=')
        .map(|(n, v)| (n.trim(), v.trim()))
        .unwrap_or((rest, "0"));

    let value: i64 = value_str.parse().map_err(|_| {
        ParseError::at(
            format!("invalid integer for stat '{}': '{}'", name, value_str),
            line,
            col,
        )
    })?;

    Ok(StatDef {
        name: name.to_string(),
        default: value,
    })
}

fn parse_flag_def(content: &str, line: usize, col: usize) -> Result<FlagDef, ParseError> {
    let rest = content.strip_prefix("flag").unwrap().trim();

    if let Some((name, value_str)) = rest.split_once('=') {
        let name = name.trim();
        let value_str = value_str.trim();
        let value = match value_str {
            "true" => true,
            "false" => false,
            _ => {
                return Err(ParseError::at(
                    format!("invalid boolean for flag '{}': '{}'", name, value_str),
                    line,
                    col,
                ));
            }
        };
        Ok(FlagDef {
            name: name.to_string(),
            default: value,
        })
    } else {
        Ok(FlagDef {
            name: rest.to_string(),
            default: false,
        })
    }
}

/// Parse an effect block: `effect name:` followed by indented body.
fn parse_effect_block(
    cursor: &mut LineCursor,
    line: usize,
    col: usize,
    header: &str,
) -> Result<(EffectDef, usize), ParseError> {
    let rest = header.strip_prefix("effect").unwrap().trim();
    let name = rest.trim_end_matches(':').trim().to_string();

    let body_indent = if cursor.has_more() {
        let next_line = cursor.peek();
        if next_line.indent > 0 {
            next_line.indent
        } else {
            return Err(ParseError::at(
                format!("effect '{}' body must be indented", name),
                line,
                col,
            ));
        }
    } else {
        return Ok((EffectDef { name, body: vec![] }, 0));
    };

    let mut body = Vec::new();

    while cursor.has_more() {
        let line = cursor.peek().clone();
        if line.content.trim().is_empty() {
            cursor.next();
            continue;
        }
        if line.indent < body_indent {
            break;
        }
        if line.indent > body_indent {
            // Deeper-indented lines we didn't consume — skip
            cursor.next();
            continue;
        }

        let step = parse_effect_step(&line.content, line.line_num, line.col)?;
        body.push(step);
        cursor.next();
    }

    Ok((EffectDef { name, body }, body_indent))
}

/// Check if a trimmed line is an effect step (vs prose text).
/// Effect steps: `+ stat by N`, `- stat by N`, `set flag to bool`,
/// `add tag`, `text "..."`. Quoted text is NOT an effect step.
fn is_effect_step(trimmed: &str) -> bool {
    trimmed.starts_with('+')
        || trimmed.starts_with('-')
        || trimmed.starts_with("set ")
        || trimmed.starts_with("add ")
        || trimmed.starts_with("text ")
}

fn parse_effect_step(content: &str, line: usize, col: usize) -> Result<EffectStep, ParseError> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(ParseError::at("empty effect step", line, col));
    }

    // Stat change: + <name> by <N>  or  - <name> by <N>
    // Also handles: + stat <name> by <N>
    if let Some(after_sign) = trimmed
        .strip_prefix('+')
        .or_else(|| trimmed.strip_prefix('-'))
    {
        let sign_val: i64 = if trimmed.starts_with('+') { 1 } else { -1 };
        let rest = after_sign.trim_start();
        let (name_part, value_str) = rest
            .split_once(" by ")
            .map(|(n, v)| (n.trim(), v.trim()))
            .ok_or_else(|| {
                ParseError::at(
                    format!("expected '<stat> by <N>' after + or -, got '{}'", trimmed),
                    line,
                    col,
                )
            })?;
        // Accept both "+ courage by 1" and "+ stat courage by 1"
        let stat_name = name_part.strip_prefix("stat ").unwrap_or(name_part).trim();
        let n: i64 = value_str
            .parse()
            .map_err(|_| ParseError::at("invalid number in stat change", line, col))?;
        return Ok(EffectStep::ChangeStat {
            stat: stat_name.to_string(),
            delta: sign_val * n,
        });
    }

    // Set flag: set <name> to true/false
    if trimmed.starts_with("set ") {
        let rest = trimmed.strip_prefix("set").unwrap().trim();
        let (name, value_str) = rest
            .split_once(" to ")
            .map(|(n, v)| (n.trim(), v.trim()))
            .ok_or_else(|| {
                ParseError::at(
                    format!("expected 'set <name> to <bool>', got '{}'", trimmed),
                    line,
                    col,
                )
            })?;
        let value = match value_str {
            "true" => true,
            "false" => false,
            _ => {
                return Err(ParseError::at(
                    format!("invalid boolean '{}', expected true or false", value_str),
                    line,
                    col,
                ));
            }
        };
        return Ok(EffectStep::SetFlag {
            flag: name.to_string(),
            value,
        });
    }

    // Add tag: add [tag] <name>
    if trimmed.starts_with("add ") {
        let rest = trimmed.strip_prefix("add").unwrap().trim();
        let tag_name = rest.strip_prefix("tag ").unwrap_or(rest).trim();
        return Ok(EffectStep::AddTag {
            tag: tag_name.to_string(),
        });
    }

    // Text output: text "..."  or bare quoted/unquoted text
    if trimmed.starts_with("text ") {
        let rest = trimmed.strip_prefix("text").unwrap().trim();
        return Ok(EffectStep::Text(parse_template_string(rest)?));
    }

    // Bare quoted or unquoted text
    Ok(EffectStep::Text(parse_template_string(trimmed)?))
}

/// Parse an event block: `event event_id:` followed by indented body.
fn parse_event_block(
    cursor: &mut LineCursor,
    line: usize,
    col: usize,
    header: &str,
) -> Result<(EventDef, usize), ParseError> {
    let rest = header.strip_prefix("event").unwrap().trim();
    let id = rest.trim_end_matches(':').trim().to_string();

    let body_indent = if cursor.has_more() {
        let next_line = cursor.peek();
        if next_line.indent > 0 {
            next_line.indent
        } else {
            return Err(ParseError::at(
                format!("event '{}' body must be indented", id),
                line,
                col,
            ));
        }
    } else {
        return Ok((
            EventDef {
                id,
                requires: None,
                tags: vec![],
                body: vec![],
                text: vec![],
                choices: vec![],
            },
            0,
        ));
    };

    let mut requires = None;
    let mut tags = Vec::new();
    let mut body = Vec::new();
    let mut text = Vec::new();
    let mut choices = Vec::new();

    while cursor.has_more() {
        let line = cursor.peek().clone();
        if line.content.trim().is_empty() {
            cursor.next();
            continue;
        }
        if line.indent < body_indent {
            break;
        }
        if line.indent > body_indent {
            cursor.next();
            continue;
        }

        let trimmed = line.content.trim_start();

        if trimmed.starts_with("requires:") {
            let rest = trimmed.strip_prefix("requires:").unwrap().trim();
            let cond = parse_condition(rest, line.line_num, line.col)?;
            requires = Some(cond);
            cursor.next();
        } else if trimmed.starts_with("tags:") {
            let rest = trimmed.strip_prefix("tags:").unwrap().trim();
            tags = rest.split(',').map(|s| s.trim().to_string()).collect();
            cursor.next();
        } else if trimmed.starts_with("choice ") || trimmed.starts_with("choice:") {
            cursor.next();
            let choice = parse_choice(cursor, line.line_num, line.col, &line.content)?;
            choices.push(choice);
        } else if is_effect_step(trimmed) {
            // Inline effect step (set flag, stat change, add tag, text)
            let step = parse_effect_step(trimmed, line.line_num, line.col)?;
            body.push(step);
            cursor.next();
        } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
            // Event prose text
            let text_content = parse_template_string(trimmed)?;
            text.push(text_content);
            cursor.next();
        } else {
            cursor.next();
        }
    }

    Ok((
        EventDef {
            id,
            requires,
            tags,
            body,
            text,
            choices,
        },
        body_indent,
    ))
}

/// Parse a choice: `choice "text":` or `choice "text" uses effect:`.
fn parse_choice(
    cursor: &mut LineCursor,
    line: usize,
    col: usize,
    header: &str,
) -> Result<ChoiceDef, ParseError> {
    let rest = header.strip_prefix("choice").unwrap();
    let rest = rest.trim_start();

    let (text_raw, remainder) = split_choice_header(rest)?;
    let text = parse_template_string(text_raw)?;

    let header_indent = col - 1;

    // Parse modifiers from the header remainder.
    // The remainder after the quoted text is: [modifers]: [next_target]
    // The `:` separates modifiers from the next target. `requires:` has its
    // own colon which is part of the keyword, not a separator.
    let mut uses = Vec::new();
    let mut requires = None;
    let mut next = None;

    // Split the remainder into modifiers part and next-target part.
    // If it starts with ':', there are no modifiers — the rest is `next`.
    let (mods_str, next_str) = if remainder.trim_start().starts_with(':') {
        let rest = remainder.trim_start();
        let after_colon = &rest[1..];
        ("", after_colon)
    } else {
        // Find the last ':' — this is the separator (not part of requires:).
        // rfind finds the LAST colon, which is the separator.
        let colon_pos = remainder.rfind(':').unwrap_or(remainder.len());
        let mods = &remainder[..colon_pos];
        let after = if colon_pos < remainder.len() {
            &remainder[colon_pos + 1..]
        } else {
            ""
        };
        (mods, after)
    };

    // Parse modifiers (uses, requires:) from mods_str
    let mods_str = mods_str.trim();
    if !mods_str.is_empty() {
        let (u, r, n) = parse_choice_modifiers(mods_str, line, col)?;
        uses = u;
        requires = r;
        next = next.or(n);
    }

    // Parse next from next_str if not already found
    let next_str = next_str.trim();
    if !next_str.is_empty() && next.is_none() {
        // next_str might be "next end" or just "end"
        let after_next = next_str
            .strip_prefix("next")
            .unwrap_or(next_str)
            .trim_start();
        let after_next = after_next
            .strip_prefix(':')
            .unwrap_or(after_next)
            .trim_start();
        let target = after_next.split_whitespace().next().unwrap_or("");
        if !target.is_empty() {
            next = Some(target.to_string());
        }
    }

    // Parse choice body
    let mut steps = Vec::new();
    let mut body_indent = None;

    while cursor.has_more() {
        let bl = cursor.peek().clone();
        if bl.content.trim().is_empty() {
            cursor.next();
            continue;
        }

        if body_indent.is_none() {
            if bl.indent > header_indent {
                body_indent = Some(bl.indent);
            } else {
                break; // No body content
            }
        }

        let expected = body_indent.unwrap();

        if bl.indent < expected {
            break;
        }
        // If bl.indent > expected, we should consume it
        // If bl.indent == expected, another body line

        let trimmed = bl.content.trim();
        cursor.next();

        // Handle uses in body
        if trimmed.starts_with("uses ") {
            let names_str = trimmed.strip_prefix("uses").unwrap().trim();
            uses.extend(names_str.split(',').map(|s| s.trim().to_string()));
            continue;
        }

        if trimmed.starts_with("next ") {
            let rest = trimmed.strip_prefix("next").unwrap().trim();
            next = Some(rest.to_string());
        } else if trimmed.starts_with("requires:") {
            let cond_str = trimmed.strip_prefix("requires:").unwrap().trim();
            requires = Some(parse_condition(cond_str, bl.line_num, bl.col)?);
        } else {
            let step = parse_effect_step(trimmed, bl.line_num, bl.col)?;
            steps.push(step)
        }
    }

    Ok(ChoiceDef {
        text,
        requires,
        steps,
        uses,
        next,
    })
}

/// Split a choice header into (text_raw, remainder) where remainder is
/// everything after the quoted/unquoted text (including the colon and modifiers).
fn split_choice_header(rest: &str) -> Result<(&str, &str), ParseError> {
    if rest.starts_with('"') {
        // Find the closing quote (handle escapes)
        let mut end = 1;
        let bytes = rest.as_bytes();
        while end < bytes.len() {
            if bytes[end] == b'"' {
                if end > 0 && bytes[end - 1] == b'\\' {
                    end += 1;
                    continue;
                }
                break;
            }
            end += 1;
        }
        let text_end = end + 1; // Include closing quote
        Ok((&rest[..text_end], &rest[text_end..]))
    } else {
        // Unquoted: find the colon
        let colon_pos = rest.find(':').unwrap_or(rest.len());
        let text = &rest[..colon_pos];
        Ok((text, &rest[colon_pos..]))
    }
}

/// Parsed choice modifiers: (uses, requires, next)
type ParsedMods = (Vec<String>, Option<ConditionExpr>, Option<String>);

/// Parse modifiers (uses, requires:, next) from the remainder string.
fn parse_choice_modifiers(
    remainder: &str,
    line: usize,
    col: usize,
) -> Result<ParsedMods, ParseError> {
    let mut uses = Vec::new();
    let mut requires = None;
    let mut next = None;
    let mut s = remainder.trim();

    while !s.is_empty() {
        s = s.trim_start();
        if s.is_empty() {
            break;
        }

        if s.starts_with("uses ") {
            let after = s.strip_prefix("uses").unwrap().trim();
            let end = find_modifier_boundary(after);
            let uses_str = after[..end].trim();
            uses.extend(uses_str.split(',').map(|x| x.trim().to_string()));
            s = after[end..].trim_start();
        } else if s.starts_with("requires:") {
            let after = s.strip_prefix("requires:").unwrap().trim();
            let end = find_modifier_boundary(after);
            let cond_str = after[..end].trim();
            requires = Some(parse_condition(cond_str, line, col)?);
            s = after[end..].trim_start();
        } else if s.starts_with("next ") {
            s = s.strip_prefix("next").unwrap().trim();
            let end = find_modifier_boundary(s);
            next = Some(s[..end].trim().to_string());
            s = s[end..].trim_start();
        } else if s.starts_with("next:") {
            s = s.strip_prefix("next:").unwrap().trim();
            let end = find_modifier_boundary(s);
            next = Some(s[..end].trim().to_string());
            s = s[end..].trim_start();
        } else {
            // Unknown modifier — skip one token
            let token_end = s.find(char::is_whitespace).unwrap_or(s.len());
            s = &s[token_end..];
        }
    }

    Ok((uses, requires, next))
}

/// Find where a modifier ends (at the next keyword: "uses" or "requires:")
fn find_modifier_boundary(s: &str) -> usize {
    let keywords = [" uses ", " uses", " requires:", "requires:"];
    let mut min_pos = s.len();
    for kw in &keywords {
        if let Some(pos) = s.find(kw) {
            if pos < min_pos {
                min_pos = pos;
            }
        }
    }
    min_pos
}

// ===== Condition parser =====

fn parse_condition(input: &str, line: usize, col: usize) -> Result<ConditionExpr, ParseError> {
    let input = input.trim();
    let mut parser = CondParser::new(input, line, col);
    let expr = parser.parse_or()?;
    parser.skip_ws();
    if !parser.input[parser.pos..].is_empty() {
        return Err(ParseError::at(
            format!(
                "unexpected trailing input in condition: '{}'",
                &parser.input[parser.pos..]
            ),
            line,
            col + parser.pos,
        ));
    }
    Ok(expr)
}

struct CondParser<'a> {
    input: &'a str,
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> CondParser<'a> {
    fn new(input: &'a str, line: usize, col: usize) -> Self {
        Self {
            input,
            pos: 0,
            line,
            col,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.input.len() && self.input[self.pos..].starts_with(char::is_whitespace)
        {
            self.pos += 1;
        }
    }

    fn try_match(&mut self, s: &str) -> bool {
        if self.input[self.pos..].starts_with(s) {
            self.pos += s.len();
            true
        } else {
            false
        }
    }

    fn parse_or(&mut self) -> Result<ConditionExpr, ParseError> {
        let mut expr = self.parse_and()?;
        self.skip_ws();
        while self.try_match("OR") {
            self.skip_ws();
            let rhs = self.parse_and()?;
            expr = ConditionExpr::Or(Box::new(expr), Box::new(rhs));
            self.skip_ws();
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<ConditionExpr, ParseError> {
        let mut expr = self.parse_not()?;
        self.skip_ws();
        while self.try_match("AND") {
            self.skip_ws();
            let rhs = self.parse_not()?;
            expr = ConditionExpr::And(Box::new(expr), Box::new(rhs));
            self.skip_ws();
        }
        Ok(expr)
    }

    fn parse_not(&mut self) -> Result<ConditionExpr, ParseError> {
        self.skip_ws();
        if self.try_match("NOT") {
            self.skip_ws();
            let inner = self.parse_not()?;
            return Ok(ConditionExpr::Not(Box::new(inner)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<ConditionExpr, ParseError> {
        self.skip_ws();
        if self.try_match("(") {
            let expr = self.parse_or()?;
            self.skip_ws();
            if !self.try_match(")") {
                return Err(ParseError::at(
                    "expected ')' to close group",
                    self.line,
                    self.col + self.pos,
                ));
            }
            return Ok(expr);
        }

        // Parse identifier
        let start = self.pos;
        while self.pos < self.input.len() {
            let c = self.input[self.pos..].chars().next().unwrap();
            if c.is_alphanumeric() || c == '_' || c == '$' {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
        let name = &self.input[start..self.pos];
        if name.is_empty() {
            return Err(ParseError::at(
                "expected condition expression",
                self.line,
                self.col + start,
            ));
        }

        self.skip_ws();

        // Check for comparison operator
        if !self.is_at_end() {
            let rest = &self.input[self.pos..];
            if let Some(op) = match_operator(rest) {
                self.pos += op.len();
                self.skip_ws();
                let num_start = self.pos;
                while self.pos < self.input.len() {
                    let c = self.input[self.pos..].chars().next().unwrap();
                    if c.is_ascii_digit() || c == '-' {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                let num_str = &self.input[num_start..self.pos];
                let value: i64 = num_str.parse().map_err(|_| {
                    ParseError::at(
                        "expected integer in condition",
                        self.line,
                        self.col + num_start,
                    )
                })?;
                return Ok(ConditionExpr::StatCompare {
                    stat: name.to_string(),
                    op: op_to_compare_op(op),
                    value,
                });
            }
            return Ok(ConditionExpr::Flag(name.to_string()));
        }
        Ok(ConditionExpr::Flag(name.to_string()))
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.input.len()
    }
}

fn match_operator(s: &str) -> Option<&'static str> {
    s.strip_prefix(">=")
        .map(|_| ">=")
        .or_else(|| s.strip_prefix("<=").map(|_| "<="))
        .or_else(|| s.strip_prefix("==").map(|_| "=="))
        .or_else(|| s.strip_prefix("!=").map(|_| "!="))
        .or_else(|| s.strip_prefix(">").map(|_| ">"))
        .or_else(|| s.strip_prefix("<").map(|_| "<"))
}

fn op_to_compare_op(op: &str) -> CompareOp {
    match op {
        ">=" => CompareOp::Gte,
        "<=" => CompareOp::Lte,
        ">" => CompareOp::Gt,
        "<" => CompareOp::Lt,
        "==" => CompareOp::Eq,
        "!=" => CompareOp::Ne,
        _ => CompareOp::Eq,
    }
}

// ===== Template / text parsing =====

/// Parse a string that may contain `{{stat}}` templates.
/// Can be quoted (with `"`) or unquoted.
fn parse_template_string(s: &str) -> Result<TextContent, ParseError> {
    let s = s.trim();

    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        let inner = &s[1..s.len() - 1];
        let segments = split_template(inner);
        Ok(TextContent { segments })
    } else {
        // Unquoted — treat as literal text
        let segments = split_template(s);
        Ok(TextContent { segments })
    }
}

/// Parse a string literal (for import paths).
fn parse_string_literal(s: &str, line: usize, col: usize) -> Result<String, ParseError> {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        let inner = &s[1..s.len() - 1];
        let mut result = String::with_capacity(inner.len());
        let mut chars = inner.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(&next) = chars.peek() {
                    match next {
                        '"' => {
                            result.push('"');
                            chars.next();
                        }
                        '\\' => {
                            result.push('\\');
                            chars.next();
                        }
                        'n' => {
                            result.push('\n');
                            chars.next();
                        }
                        't' => {
                            result.push('\t');
                            chars.next();
                        }
                        'r' => {
                            result.push('\r');
                            chars.next();
                        }
                        _ => {
                            result.push('\\');
                            result.push(next);
                            chars.next();
                        }
                    }
                } else {
                    result.push('\\');
                }
            } else {
                result.push(c);
            }
        }
        Ok(result)
    } else {
        Err(ParseError::at(
            format!("expected quoted string: '{}'", s),
            line,
            col,
        ))
    }
}

/// Split text into literal and stat-reference segments.
/// `{{stat_name}}` → StatRef("stat_name")
/// Everything else → Literal("...")
fn split_template(text: &str) -> Vec<TextSegment> {
    let mut segments = Vec::new();
    let mut current_literal = String::new();

    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' && chars.peek() == Some(&'{') {
            chars.next(); // consume second `{`

            if !current_literal.is_empty() {
                segments.push(TextSegment::Literal(std::mem::take(&mut current_literal)));
            }

            let mut var_name = String::new();
            loop {
                if let Some(&c) = chars.peek() {
                    if c == '}' {
                        chars.next();
                        if chars.peek() == Some(&'}') {
                            chars.next();
                            break;
                        } else {
                            var_name.push('}');
                        }
                    } else {
                        chars.next();
                        var_name.push(c);
                    }
                } else {
                    segments.push(TextSegment::Literal(format!("{{{{{}", var_name)));
                    break;
                }
            }

            let var_name = var_name.trim().to_string();
            if var_name.is_empty() {
                current_literal.push_str("{{}}");
            } else {
                segments.push(TextSegment::StatRef(var_name));
            }
        } else {
            current_literal.push(c);
        }
    }

    if !current_literal.is_empty() {
        segments.push(TextSegment::Literal(current_literal));
    }

    segments
}
