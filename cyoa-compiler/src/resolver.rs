//! Import resolver — resolves and merges imported `.cyoa` files.
//!
//! The compiler uses 3 passes:
//! 1. Parse pass: parse the main file + all transitively-referenced files
//! 2. Merge pass: build dependency graph, detect cycles, resolve collisions
//! 3. Codegen pass: emit unified bytecode (see `codegen.rs`)
//!
//! This module handles passes 1 and 2.

use crate::parser::parse_story;
use crate::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Error during import resolution.
#[derive(Debug)]
pub enum ImportError {
    /// File not found at resolved path.
    FileNotFound(PathBuf),
    /// Circular import detected: `cycle` is the import path cycle.
    CircularImport(Vec<String>),
    /// Name collision: the same name is defined in multiple imports.
    NameCollision {
        name: String,
        file_a: String,
        file_b: String,
    },
    /// Parse error in an imported file.
    Parse(crate::parser::ParseError),
    /// IO error reading a file.
    Io(std::io::Error),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::FileNotFound(path) => {
                write!(f, "file not found: {}", path.display())
            }
            ImportError::CircularImport(cycle) => {
                write!(f, "circular import detected: {}", cycle.join(" -> "))
            }
            ImportError::NameCollision {
                name,
                file_a,
                file_b,
            } => {
                write!(
                    f,
                    "name '{}' defined in both '{}' and '{}'",
                    name, file_a, file_b
                )
            }
            ImportError::Parse(e) => write!(f, "parse error in import: {}", e),
            ImportError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for ImportError {}

impl From<std::io::Error> for ImportError {
    fn from(e: std::io::Error) -> Self {
        ImportError::Io(e)
    }
}

impl From<crate::parser::ParseError> for ImportError {
    fn from(e: crate::parser::ParseError) -> Self {
        ImportError::Parse(e)
    }
}

/// Resolve all imports in a story, returning a merged, self-contained `Story`.
///
/// - `base_dir`: the directory of the main `.cyoa` file (for `./` imports)
/// - `std_paths`: directories to search for `std/` imports
///
/// The returned `Story` has no `Import` items — all definitions are merged
/// into a single flat namespace.
pub fn resolve_imports(
    story: &Story,
    base_dir: &Path,
    std_paths: &[PathBuf],
) -> Result<Story, ImportError> {
    let mut resolver = ImportResolver {
        std_paths,
        visited: BTreeSet::new(),
        stat_names: BTreeMap::new(),
        flag_names: BTreeMap::new(),
        effect_names: BTreeMap::new(),
        event_names: BTreeMap::new(),
    };

    let mut merged_items = Vec::new();

    // Process all items in the story, resolving imports recursively
    for item in &story.items {
        match item {
            StoryItem::Import(import) => {
                let file_path = resolver.resolve_path(&import.path, base_dir, &import.alias)?;
                let file_story = resolver.load_and_parse(&file_path, base_dir, std_paths)?;
                let resolved = resolver.resolve_imports_recursive(
                    &file_story,
                    file_path.parent().unwrap_or(base_dir),
                )?;
                for sub_item in resolved.items {
                    resolver.check_collision(
                        &sub_item,
                        &file_path.display().to_string(),
                        "main",
                    )?;
                    merged_items.push(sub_item);
                }
                // Also merge top-level definitions from the imported file
            }
            _ => {
                // Non-import items: check for collisions and add directly
                resolver.check_collision(item, "main", "main")?;
                merged_items.push(item.clone());
            }
        }
    }

    // Process imports from the original story's imports list
    for import in &story.imports {
        let file_path = resolver.resolve_path(&import.path, base_dir, &import.alias)?;
        if resolver.visited.contains(&file_path) {
            // Already processed (imported by a previous file) — skip
            continue;
        }
        resolver.visited.insert(file_path.clone());

        let file_story = resolver.load_and_parse(&file_path, base_dir, std_paths)?;
        let resolved = resolver
            .resolve_imports_recursive(&file_story, file_path.parent().unwrap_or(base_dir))?;

        for sub_item in resolved.items {
            // Skip Import items (they've been resolved)
            if matches!(sub_item, StoryItem::Import(_)) {
                continue;
            }
            resolver.check_collision(&sub_item, &file_path.display().to_string(), "main")?;
            merged_items.push(sub_item);
        }
    }

    Ok(Story {
        name: story.name.clone(),
        imports: Vec::new(), // All resolved — no imports remain
        tags: story.tags.clone(),
        items: merged_items,
    })
}

struct ImportResolver<'a> {
    std_paths: &'a [PathBuf],
    visited: BTreeSet<PathBuf>,
    stat_names: BTreeMap<String, String>, // name → source file
    flag_names: BTreeMap<String, String>,
    effect_names: BTreeMap<String, String>,
    event_names: BTreeMap<String, String>,
}

impl<'a> ImportResolver<'a> {
    /// Resolve an import path to a file system path.
    fn resolve_path(
        &self,
        path: &str,
        base_dir: &Path,
        _alias: &Option<String>,
    ) -> Result<PathBuf, ImportError> {
        if let Some(rel_path) = path.strip_prefix("std/") {
            // std/ imports resolve from std_paths (which point to the std/ directory itself)
            let rel = Path::new(rel_path);
            for std_base in self.std_paths {
                let full = std_base.join(rel).with_extension("cyoa");
                if full.exists() {
                    return Ok(full);
                }
            }
            // Try without .cyoa extension
            for std_base in self.std_paths {
                let full = std_base.join(rel);
                if full.exists() {
                    return Ok(full);
                }
            }
            Err(ImportError::FileNotFound(PathBuf::from(path)))
        } else if path.starts_with("./") || path.starts_with("../") {
            // Relative path — base_dir is the directory of the file doing the import
            let rel = Path::new(&path[2..]); // Strip "./"
            let full = base_dir.join(rel).with_extension("cyoa");
            if full.exists() {
                Ok(full)
            } else {
                Err(ImportError::FileNotFound(full))
            }
        } else {
            // Absolute path or unknown scheme
            Err(ImportError::FileNotFound(PathBuf::from(path)))
        }
    }

    /// Load and parse a file from the filesystem.
    fn load_and_parse(
        &self,
        path: &Path,
        base_dir: &Path,
        std_paths: &[PathBuf],
    ) -> Result<Story, ImportError> {
        let _ = (base_dir, std_paths); // available for future multi-root resolution
        let source = std::fs::read_to_string(path)?;
        let story = parse_story(&source)?;
        Ok(story)
    }

    /// Recursively resolve imports in a parsed story.
    fn resolve_imports_recursive(
        &mut self,
        story: &Story,
        file_path: &Path,
    ) -> Result<Story, ImportError> {
        let mut merged_items = Vec::new();

        // First, process items from this story
        for item in &story.items {
            if matches!(item, StoryItem::Import(_)) {
                continue; // Skip import items — resolved separately
            }
            merged_items.push(item.clone());
        }

        // Process imports from this story
        for import in &story.imports {
            let resolved_path = self.resolve_path(
                &import.path,
                file_path.parent().unwrap_or(file_path),
                &import.alias,
            )?;

            if self.visited.contains(&resolved_path) {
                continue; // Already merged
            }
            self.visited.insert(resolved_path.clone());

            let imported_story = self.load_and_parse(
                &resolved_path,
                file_path.parent().unwrap_or(file_path),
                self.std_paths,
            )?;
            let resolved = self.resolve_imports_recursive(&imported_story, &resolved_path)?;

            for sub_item in resolved.items {
                if matches!(sub_item, StoryItem::Import(_)) {
                    continue;
                }
                self.check_collision(&sub_item, &resolved_path.display().to_string(), "import")?;
                merged_items.push(sub_item);
            }
        }

        Ok(Story {
            name: story.name.clone(),
            imports: Vec::new(),
            tags: story.tags.clone(),
            items: merged_items,
        })
    }

    /// Check if an item's name collides with an existing definition.
    fn check_collision(
        &mut self,
        item: &StoryItem,
        source: &str,
        _context: &str,
    ) -> Result<(), ImportError> {
        match item {
            StoryItem::StatDef(s) => {
                if let Some(prev) = self.stat_names.get(&s.name) {
                    if prev != source {
                        return Err(ImportError::NameCollision {
                            name: s.name.clone(),
                            file_a: prev.clone(),
                            file_b: source.to_string(),
                        });
                    }
                }
                self.stat_names.insert(s.name.clone(), source.to_string());
            }
            StoryItem::FlagDef(f) => {
                if let Some(prev) = self.flag_names.get(&f.name) {
                    if prev != source {
                        return Err(ImportError::NameCollision {
                            name: f.name.clone(),
                            file_a: prev.clone(),
                            file_b: source.to_string(),
                        });
                    }
                }
                self.flag_names.insert(f.name.clone(), source.to_string());
            }
            StoryItem::EffectDef(e) => {
                if let Some(prev) = self.effect_names.get(&e.name) {
                    if prev != source {
                        return Err(ImportError::NameCollision {
                            name: e.name.clone(),
                            file_a: prev.clone(),
                            file_b: source.to_string(),
                        });
                    }
                }
                self.effect_names.insert(e.name.clone(), source.to_string());
            }
            StoryItem::EventDef(ev) => {
                if let Some(prev) = self.event_names.get(&ev.id) {
                    if prev != source {
                        return Err(ImportError::NameCollision {
                            name: ev.id.clone(),
                            file_a: prev.clone(),
                            file_b: source.to_string(),
                        });
                    }
                }
                self.event_names.insert(ev.id.clone(), source.to_string());
            }
            StoryItem::Import(_) => {}
        }
        Ok(())
    }
}
