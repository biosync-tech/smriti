use console::style;
use dialoguer::{Confirm, FuzzySelect, Input, MultiSelect};

use crate::errors::AppResult;
use crate::models::*;
use crate::parser;
use crate::storage::Database;

/// Default tag choices offered during interactive note creation.
const DEFAULT_TAGS: &[&str] = &[
    "client", "project", "decision", "sop", "research", "idea", "daily",
];

/// Relationship types for linking notes.
const RELATION_TYPES: &[&str] = &[
    "causal", "semantic", "temporal", "inspires", "uses", "blocks",
];

/// Walk the user through creating a note interactively.
///
/// Prompts for title, content, tags, and optional links to existing notes.
/// Shows a preview and asks for confirmation before persisting.
pub fn interactive_new(db: &Database) -> AppResult<()> {
    // 1. Title
    let title: String = Input::new()
        .with_prompt("Title")
        .validate_with(|input: &String| -> Result<(), String> {
            if input.trim().len() < 2 {
                Err("Title must be at least 2 characters".into())
            } else {
                Ok(())
            }
        })
        .interact_text()
        .map_err(|e| crate::errors::AppError::BadRequest(e.to_string()))?;

    // 2. Content
    let content: String = Input::new()
        .with_prompt("Content (markdown, single line — use 'create --file' for longer notes)")
        .allow_empty(true)
        .interact_text()
        .map_err(|e| crate::errors::AppError::BadRequest(e.to_string()))?;

    // 3. Tags — multi-select from defaults, plus custom entry
    let tag_selections = MultiSelect::new()
        .with_prompt("Tags (space to select, enter to confirm)")
        .items(DEFAULT_TAGS)
        .interact()
        .map_err(|e| crate::errors::AppError::BadRequest(e.to_string()))?;

    let mut tags: Vec<String> = tag_selections
        .iter()
        .map(|&i| DEFAULT_TAGS[i].to_string())
        .collect();

    // Allow adding custom tags
    let custom: String = Input::new()
        .with_prompt("Additional tags (comma-separated, or enter to skip)")
        .allow_empty(true)
        .interact_text()
        .map_err(|e| crate::errors::AppError::BadRequest(e.to_string()))?;

    for t in custom.split(',').map(|s| s.trim().to_string()) {
        if !t.is_empty() && !tags.contains(&t) {
            tags.push(t);
        }
    }

    // Also extract inline tags from content
    for t in parser::extract_tags(&content) {
        if !tags.contains(&t) {
            tags.push(t);
        }
    }

    // 4. Links — optional fuzzy search against existing notes
    let mut link_target: Option<(String, String)> = None; // (id, title)
    let mut link_relation: Option<String> = None;

    let wants_link = Confirm::new()
        .with_prompt("Link to an existing note?")
        .default(false)
        .interact()
        .map_err(|e| crate::errors::AppError::BadRequest(e.to_string()))?;

    if wants_link {
        let notes = db.list_notes(&NoteListQuery {
            limit: 200,
            offset: 0,
            sort: SortOrder::UpdatedDesc,
            tag: None,
        })?;

        if notes.is_empty() {
            println!("  No existing notes to link to.");
        } else {
            let titles: Vec<String> = notes.iter().map(|n| n.title.clone()).collect();

            let selection: Option<usize> = FuzzySelect::new()
                .with_prompt("Search notes")
                .items(&titles)
                .interact_opt()
                .map_err(|e: dialoguer::Error| crate::errors::AppError::BadRequest(e.to_string()))?;

            if let Some(idx) = selection {
                link_target = Some((notes[idx].id.clone(), notes[idx].title.clone()));

                // 5. Relationship type
                let rel_idx: usize = FuzzySelect::new()
                    .with_prompt("Relationship type")
                    .items(RELATION_TYPES)
                    .default(0)
                    .interact()
                    .map_err(|e: dialoguer::Error| crate::errors::AppError::BadRequest(e.to_string()))?;

                link_relation = Some(RELATION_TYPES[rel_idx].to_string());
            }
        }
    }

    // 6. Preview and confirm
    println!();
    println!("{}", style("── Note Preview ──").bold());
    println!("  Title:   {}", style(&title).cyan());
    if !content.is_empty() {
        let preview = if content.len() > 120 {
            format!("{}...", crate::safe_truncate(&content, 117))
        } else {
            content.clone()
        };
        println!("  Content: {}", preview);
    }
    if !tags.is_empty() {
        let tag_display: Vec<String> = tags.iter().map(|t| format!("#{}", t)).collect();
        println!("  Tags:    {}", style(tag_display.join(" ")).yellow());
    }
    if let Some((_, ref target_title)) = link_target {
        println!(
            "  Link:    {} → {}",
            style(link_relation.as_deref().unwrap_or("wikilink")).green(),
            style(target_title).cyan()
        );
    }
    println!();

    let confirmed = Confirm::new()
        .with_prompt("Create this note?")
        .default(true)
        .interact()
        .map_err(|e| crate::errors::AppError::BadRequest(e.to_string()))?;

    if !confirmed {
        println!("{}", style("Cancelled — nothing was saved.").dim());
        return Ok(());
    }

    // Create the note
    let note = db.create_note(CreateNoteRequest {
        title,
        content: content.clone(),
        tags,
    })?;

    // Process wikilinks from content
    let wikilinks = parser::extract_wikilinks(&content);
    for wl in &wikilinks {
        if let Ok(Some(target)) = db.get_note_by_title(&wl.target) {
            let _ = db.create_link(&note.id, &target.id, LinkType::WikiLink);
        }
    }

    // Create the explicit link the user chose
    if let (Some((target_id, _)), Some(relation)) = (link_target, link_relation) {
        let link_type = match relation.as_str() {
            "causal" => LinkType::Causal,
            "semantic" => LinkType::Semantic,
            "temporal" => LinkType::Temporal,
            _ => LinkType::Custom(relation),
        };
        let _ = db.create_link(&note.id, &target_id, link_type);
    }

    println!(
        "{} {} (id: {})",
        style("✓ Created:").green().bold(),
        style(&note.title).cyan(),
        &note.id[..8]
    );

    Ok(())
}
