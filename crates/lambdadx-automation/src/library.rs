//! Metadata-only search and lazy file reads, executed on a dedicated worker.
use crate::contract::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

pub const SKILL: &str = include_str!("../../../maimai-note-desinger-skill/SKILL.md");
const MAX_INDEX_BYTES: usize = 32 * 1024 * 1024;
const MAX_ENTRIES: usize = 50_000;
pub const MAX_LIBRARY_TEXT: usize = 8192;
const MAX_DEFINITIONS: usize = 8;
const MAX_DEFINITION_BYTES: usize = 48 * 1024;

/// Resolve named configurations and their complete definitions in one worker call.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibraryResolveRequest {
    pub schema_version: u32,
    pub prompt: String,
    /// Newest first; consulted only when the current prompt names no configuration.
    #[serde(default)]
    pub previous_prompts: Vec<String>,
}
impl LibraryResolveRequest {
    pub fn validate(&self) -> ToolResult<()> {
        if self.previous_prompts.len() > 8
            || self.prompt.len() + self.previous_prompts.iter().map(String::len).sum::<usize>()
                > 64 * 1024
        {
            return Err(invalid(
                "Resolve accepts at most 8 previous prompts and 64 KiB of prompt text",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub id: String,
    pub name: String,
    pub completeness: String,
    pub text: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolvedLibrary {
    pub revision: String,
    pub definitions: Vec<Definition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibrarySearchRequest {
    pub schema_version: u32,
    pub query: String,
    #[serde(default)]
    pub offset: usize,
    pub limit: usize,
    pub revision: Option<String>,
}
impl LibrarySearchRequest {
    pub fn validate(&self) -> ToolResult<()> {
        if self.query.len() > 256
            || !(1..=20).contains(&self.limit)
            || self.revision.as_ref().is_some_and(|s| s.len() > 128)
        {
            return Err(ToolError::invalid(
                "Library query <=256 bytes; limit 1..20; revision <=128 bytes",
            ));
        }
        if self.offset > 0 && self.revision.is_none() {
            return Err(ToolError::invalid("Library pagination requires revision"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibraryReadRequest {
    pub schema_version: u32,
    pub revision: String,
    pub id: String,
    /// Omit for entry body; otherwise a reference ID from library_search.
    pub part: Option<String>,
    /// UTF-8 byte cursor copied from next_offset, on a codepoint boundary.
    #[serde(default)]
    pub offset: u64,
    pub max_bytes: usize,
    pub file_revision: Option<String>,
}
impl LibraryReadRequest {
    pub fn validate(&self) -> ToolResult<()> {
        if !valid_id(&self.id)
            || self.part.as_ref().is_some_and(|s| !valid_id(s))
            || self.revision.is_empty()
            || self.revision.len() > 128
            || !(4..=MAX_LIBRARY_TEXT).contains(&self.max_bytes)
        {
            return Err(ToolError::invalid(
                "Invalid library ID/revision; max_bytes must be 4..8192",
            ));
        }
        if (self.offset > 0 && self.file_revision.is_none())
            || self.file_revision.as_ref().is_some_and(|s| s.len() > 128)
        {
            return Err(ToolError::invalid(
                "Continuation requires file_revision from the preceding text page",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FileRef {
    pub path: String,
    pub bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub file: FileRef,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub tags: Vec<String>,
    pub summary: String,
    pub completeness: String,
    pub source: String,
    pub body: FileRef,
    pub references: Vec<Reference>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Hit {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub tags: Vec<String>,
    pub summary: String,
    pub completeness: String,
    pub source: String,
    pub references: Vec<ReferenceInfo>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReferenceInfo {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchPage {
    pub revision: String,
    pub hits: Vec<Hit>,
    pub total: usize,
    pub next_offset: Option<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TextPage {
    pub revision: String,
    pub file_revision: String,
    pub id: String,
    pub part: String,
    pub text: String,
    pub offset: u64,
    pub next_offset: Option<u64>,
    pub total_bytes: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    schema_version: u32,
    revision: String,
    entries: Vec<Entry>,
}
struct Loaded {
    stamp: (u64, u128),
    index: Index,
    search: Vec<String>,
    names: Vec<Vec<String>>,
}
pub struct Library {
    root: PathBuf,
    loaded: Option<Loaded>,
}
fn invalid(error: impl std::fmt::Display) -> ToolError {
    ToolError::invalid(format!("Library: {error}"))
}
fn changed() -> ToolError {
    ToolError::new(
        ErrorCode::DocumentChanged,
        "Library changed; rebuild its index and search again",
    )
}
fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 96
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
fn stamp(meta: &fs::Metadata) -> ToolResult<(u64, u128)> {
    if !meta.is_file() {
        return Err(invalid("Expected a regular library file"));
    }
    Ok((
        meta.len(),
        meta.modified()
            .map_err(invalid)?
            .duration_since(UNIX_EPOCH)
            .map_err(invalid)?
            .as_nanos(),
    ))
}
fn local_path(root: &Path, relative: &str) -> ToolResult<PathBuf> {
    let path = Path::new(relative);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(invalid("Expected a relative path without traversal"));
    }
    let resolved = root.join(path).canonicalize().map_err(invalid)?;
    if !resolved.starts_with(root) {
        return Err(invalid("Reference escapes the library root"));
    }
    Ok(resolved)
}
impl Library {
    /// Discovery happens on the worker, never on the editor frame thread.
    pub fn discover() -> Self {
        let root = std::env::var_os("LDX_NOTE_DESIGN_LIBRARY")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::current_dir()
                    .ok()?
                    .ancestors()
                    .map(|p| p.join("maimai-note-desinger-skill"))
                    .find(|p| p.join("catalog.json").is_file())
            })
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../maimai-note-desinger-skill")
            });
        Self::new(root)
    }
    pub fn new(root: PathBuf) -> Self {
        Self { root, loaded: None }
    }
    fn load(&mut self) -> ToolResult<&Loaded> {
        self.root = self.root.canonicalize().map_err(|_| {
            invalid("Library not found; set LDX_NOTE_DESIGN_LIBRARY to the skill directory")
        })?;
        let path = local_path(&self.root, "catalog.json")?;
        let file = File::open(path).map_err(invalid)?;
        let current = stamp(&file.metadata().map_err(invalid)?)?;
        if self.loaded.as_ref().is_none_or(|l| l.stamp != current) {
            if current.0 > MAX_INDEX_BYTES as u64 {
                return Err(invalid("Index exceeds 32 MiB"));
            }
            let mut bytes = Vec::new();
            file.take(MAX_INDEX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(invalid)?;
            if bytes.len() > MAX_INDEX_BYTES {
                return Err(invalid("Index exceeds 32 MiB"));
            }
            let index: Index = serde_json::from_slice(&bytes).map_err(invalid)?;
            if index.schema_version != 1
                || index.revision.is_empty()
                || index.revision.len() > 128
                || index.entries.len() > MAX_ENTRIES
            {
                return Err(invalid("Unsupported or oversized library index"));
            }
            let mut ids = HashSet::new();
            for e in &index.entries {
                if !valid_id(&e.id)
                    || !ids.insert(&e.id)
                    || e.name.len() > 128
                    || e.summary.len() > 512
                    || e.aliases.len() > 16
                    || e.tags.len() > 16
                    || e.aliases.iter().chain(&e.tags).any(|s| s.len() > 128)
                    || e.source.len() > 256
                    || !matches!(
                        e.completeness.as_str(),
                        "complete" | "partial" | "name_only"
                    )
                    || e.references.len() > 32
                {
                    return Err(invalid("Invalid or oversized catalog entry"));
                }
                let mut refs = HashSet::new();
                for r in &e.references {
                    if !valid_id(&r.id)
                        || r.id == "entry"
                        || !refs.insert(&r.id)
                        || r.label.len() > 128
                        || r.kind.len() > 32
                    {
                        return Err(invalid("Invalid reference metadata"));
                    }
                }
            }
            let search = index
                .entries
                .iter()
                .map(|e| {
                    format!(
                        "{} {} {} {}",
                        e.name,
                        e.aliases.join(" "),
                        e.tags.join(" "),
                        e.summary
                    )
                    .to_lowercase()
                })
                .collect();
            let names = index
                .entries
                .iter()
                .map(|e| {
                    std::iter::once(&e.id)
                        .chain(std::iter::once(&e.name))
                        .chain(&e.aliases)
                        .filter(|s| !s.trim().is_empty())
                        .map(|s| s.to_lowercase())
                        .collect()
                })
                .collect();
            self.loaded = Some(Loaded {
                stamp: current,
                index,
                search,
                names,
            });
        }
        Ok(self.loaded.as_ref().unwrap())
    }
    /// Match names/aliases in natural language, favoring the most specific name
    /// at each position. Neither unrelated bodies nor reference Simai are opened.
    pub fn resolve(&mut self, r: &LibraryResolveRequest) -> ToolResult<ResolvedLibrary> {
        r.validate()?;
        let loaded = self.load()?;
        let mut selected = Vec::new();
        for prompt in std::iter::once(&r.prompt).chain(&r.previous_prompts) {
            let prompt = prompt.to_lowercase();
            let mut matches = Vec::new();
            for (i, names) in loaded.names.iter().enumerate() {
                for name in names {
                    for (start, _) in prompt.match_indices(name).take(16) {
                        let end = start + name.len();
                        // English IDs must be whole words; Chinese needs substring matching.
                        if name.is_ascii()
                            && (prompt[..start]
                                .chars()
                                .next_back()
                                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '-')
                                || prompt[end..]
                                    .chars()
                                    .next()
                                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '-'))
                        {
                            continue;
                        }
                        matches.push((start, end, i));
                        if matches.len() > 4096 {
                            return Err(invalid(
                                "Too many matching configurations; narrow the prompt",
                            ));
                        }
                    }
                }
            }
            matches.sort_unstable_by_key(|&(start, end, i)| {
                (std::cmp::Reverse(end - start), start, i)
            });
            let mut spans = Vec::new();
            for (start, end, i) in matches {
                if spans
                    .iter()
                    .any(|&(a, b)| a <= start && end <= b && b - a > end - start)
                {
                    continue;
                }
                spans.push((start, end));
                if !selected.contains(&i) {
                    selected.push(i);
                }
                if selected.len() > MAX_DEFINITIONS {
                    return Err(invalid(
                        "More than 8 configurations matched; narrow the prompt or use library_search/library_read",
                    ));
                }
            }
            if !selected.is_empty() {
                break;
            }
        }
        let revision = loaded.index.revision.clone();
        let entries: Vec<_> = selected
            .into_iter()
            .map(|i| loaded.index.entries[i].clone())
            .collect();
        let mut result = ResolvedLibrary {
            revision: revision.clone(),
            definitions: Vec::new(),
        };
        let mut remaining = MAX_DEFINITION_BYTES;
        for entry in entries {
            if entry.body.bytes > remaining as u64 {
                return Err(invalid(
                    "Complete definitions exceed 48 KiB; narrow the prompt or use library_read",
                ));
            }
            let mut request = LibraryReadRequest {
                schema_version: SCHEMA_VERSION,
                revision: revision.clone(),
                id: entry.id.clone(),
                part: None,
                offset: 0,
                max_bytes: MAX_LIBRARY_TEXT,
                file_revision: None,
            };
            let mut text = String::new();
            loop {
                let page = self.read(&request)?;
                text.push_str(&page.text);
                let Some(offset) = page.next_offset else {
                    break;
                };
                request.offset = offset;
                request.file_revision = Some(page.file_revision);
            }
            remaining -= text.len();
            result.definitions.push(Definition {
                id: entry.id,
                name: entry.name,
                completeness: entry.completeness,
                text,
            });
        }
        if serde_json::to_vec(&result).map_err(invalid)?.len() > MAX_RESULT_BYTES {
            return Err(invalid(
                "Serialized definitions exceed the result budget; narrow the prompt",
            ));
        }
        Ok(result)
    }
    pub fn search(&mut self, r: &LibrarySearchRequest) -> ToolResult<SearchPage> {
        r.validate()?;
        let loaded = self.load()?;
        if r.revision
            .as_ref()
            .is_some_and(|rev| *rev != loaded.index.revision)
        {
            return Err(changed());
        }
        let query = r.query.trim().to_lowercase();
        let terms: Vec<_> = query.split_whitespace().collect();
        let mut matches: Vec<_> = loaded
            .index
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if terms.iter().all(|t| loaded.search[i].contains(t)) {
                    let exact = e.name.to_lowercase() == query
                        || e.aliases.iter().any(|a| a.to_lowercase() == query);
                    Some((!exact, &e.id, i))
                } else {
                    None
                }
            })
            .collect();
        matches.sort_unstable();
        let total = matches.len();
        let mut hits = matches
            .into_iter()
            .skip(r.offset)
            .take(r.limit)
            .map(|(_, _, i)| {
                let e = &loaded.index.entries[i];
                Hit {
                    id: e.id.clone(),
                    name: e.name.clone(),
                    aliases: e.aliases.clone(),
                    tags: e.tags.clone(),
                    summary: e.summary.clone(),
                    completeness: e.completeness.clone(),
                    source: e.source.clone(),
                    references: e
                        .references
                        .iter()
                        .map(|r| ReferenceInfo {
                            id: r.id.clone(),
                            label: r.label.clone(),
                            kind: r.kind.clone(),
                            bytes: r.file.bytes,
                        })
                        .collect(),
                }
            })
            .collect::<Vec<_>>();
        while serde_json::to_vec(&hits).map_err(invalid)?.len() > 48 * 1024 {
            hits.pop();
        }
        if hits.is_empty() && r.offset < total {
            return Err(invalid(
                "A catalog entry exceeds the result budget; shorten its metadata",
            ));
        }
        Ok(SearchPage {
            revision: loaded.index.revision.clone(),
            next_offset: (r.offset.saturating_add(hits.len()) < total)
                .then_some(r.offset + hits.len()),
            hits,
            total,
        })
    }
    pub fn read(&mut self, r: &LibraryReadRequest) -> ToolResult<TextPage> {
        r.validate()?;
        let loaded = self.load()?;
        if r.revision != loaded.index.revision {
            return Err(changed());
        }
        let entry = loaded
            .index
            .entries
            .iter()
            .find(|e| e.id == r.id)
            .ok_or_else(|| invalid("Unknown entry ID"))?;
        let part = r.part.as_deref().unwrap_or("entry");
        let reference = if part == "entry" {
            &entry.body
        } else {
            &entry
                .references
                .iter()
                .find(|s| s.id == part)
                .ok_or_else(|| invalid("Unknown reference ID"))?
                .file
        }
        .clone();
        let mut file = File::open(local_path(&self.root, &reference.path)?).map_err(invalid)?;
        let before = stamp(&file.metadata().map_err(invalid)?)?;
        let file_revision = format!("{}-{}", before.0, before.1);
        if before.0 != reference.bytes
            || r.file_revision
                .as_ref()
                .is_some_and(|v| *v != file_revision)
        {
            return Err(changed());
        }
        if r.offset > before.0 {
            return Err(invalid("Offset exceeds file size"));
        }
        file.seek(SeekFrom::Start(r.offset)).map_err(invalid)?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(r.max_bytes as u64)
            .read_to_end(&mut bytes)
            .map_err(invalid)?;
        if let Err(error) = std::str::from_utf8(&bytes) {
            if error.error_len().is_none() && r.offset + (bytes.len() as u64) < before.0 {
                bytes.truncate(error.valid_up_to());
            } else {
                return Err(invalid(
                    "Invalid UTF-8 or byte cursor is not a codepoint boundary",
                ));
            }
        }
        if stamp(&file.metadata().map_err(invalid)?)? != before {
            return Err(changed());
        }
        let end = r.offset + bytes.len() as u64;
        Ok(TextPage {
            revision: r.revision.clone(),
            file_revision,
            id: r.id.clone(),
            part: part.into(),
            text: String::from_utf8(bytes).map_err(invalid)?,
            offset: r.offset,
            next_offset: (end < before.0).then_some(end),
            total_bytes: before.0,
        })
    }
    pub(crate) fn execute(&mut self, request: Request) -> ToolResult<serde_json::Value> {
        match request {
            Request::LibraryResolve(r) => serde_json::to_value(self.resolve(&r)?).map_err(invalid),
            Request::LibrarySearch(r) => serde_json::to_value(self.search(&r)?).map_err(invalid),
            Request::LibraryRead(r) => serde_json::to_value(self.read(&r)?).map_err(invalid),
            _ => Err(invalid("Not a library request")),
        }
    }
}

#[cfg(test)]
mod tests;
