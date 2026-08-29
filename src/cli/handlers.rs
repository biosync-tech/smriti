use std::collections::HashMap;
use std::path::Path;

use crate::errors::AppResult;
use crate::graph::KnowledgeGraph;
use crate::models::*;
use crate::parser;
use crate::storage::Database;

/// Run a closure and show a spinner if it takes longer than 50ms.
///
/// When the `interactive` feature is disabled, the closure runs without any
/// visual feedback. The `success_msg` closure receives the result and returns
/// the text shown after the spinner completes.
#[cfg(feature = "interactive")]
fn with_spinner<T, F, M>(message: &str, f: F, success_msg: M) -> AppResult<T>
where
    F: FnOnce() -> AppResult<T>,
    M: FnOnce(&T) -> String,
{
    use indicatif::{ProgressBar, ProgressStyle};
    use std::time::{Duration, Instant};

    let start = Instant::now();
    let result = f()?;
    let elapsed = start.elapsed();

    if elapsed > Duration::from_millis(50) {
        // Operation was slow — retroactively show a completed spinner line
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("{spinner:.green} {msg}")
                .expect("valid template")
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✓"]),
        );
        pb.set_message(message.to_string());
        pb.finish_with_message(success_msg(&result));
    }

    Ok(result)
}

#[cfg(not(feature = "interactive"))]
fn with_spinner<T, F, M>(_message: &str, f: F, _success_msg: M) -> AppResult<T>
where
    F: FnOnce() -> AppResult<T>,
    M: FnOnce(&T) -> String,
{
    f()
}

/// Handle `notes create`
pub fn handle_create(
    db: &Database,
    title: String,
    content: Option<String>,
    file: Option<String>,
    tags: Option<Vec<String>>,
) -> AppResult<()> {
    let content = if let Some(file_path) = file {
        std::fs::read_to_string(&file_path)?
    } else {
        content.unwrap_or_default()
    };

    let mut all_tags = tags.unwrap_or_default();
    // Auto-extract tags from content
    for tag in parser::extract_tags(&content) {
        if !all_tags.contains(&tag) {
            all_tags.push(tag);
        }
    }

    let note = with_spinner(
        "Creating note...",
        || {
            db.create_note(CreateNoteRequest {
                title,
                content: content.clone(),
                tags: all_tags,
            })
        },
        |n| format!("Created: \"{}\" (id: {})", n.title, &n.id[..8]),
    )?;

    // Process wiki-links
    let wikilinks = parser::extract_wikilinks(&content);
    for wl in &wikilinks {
        if let Ok(Some(target)) = db.get_note_by_title(&wl.target) {
            let _ = db.create_link(&note.id, &target.id, LinkType::WikiLink);
        }
    }

    println!("Created note: {} ({})", note.title, note.id);
    if !note.tags.is_empty() {
        println!("  Tags: {}", note.tags.join(", "));
    }
    if !wikilinks.is_empty() {
        println!(
            "  Wiki-links: {}",
            wikilinks
                .iter()
                .map(|w| w.target.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    Ok(())
}

/// Handle `notes read`
pub fn handle_read(db: &Database, id: String, json: bool) -> AppResult<()> {
    // Try by ID first, then by title
    let note = match db.get_note(&id) {
        Ok(n) => n,
        Err(_) => match db.get_note_by_title(&id)? {
            Some(n) => db.get_note(&n.id)?,
            None => return Err(crate::errors::AppError::NoteNotFound(id)),
        },
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&note)?);
    } else {
        println!("━━━ {} ━━━", note.title);
        println!("ID: {}", note.id);
        println!(
            "Created: {} | Updated: {}",
            note.created_at.format("%Y-%m-%d %H:%M"),
            note.updated_at.format("%Y-%m-%d %H:%M")
        );
        if !note.tags.is_empty() {
            println!(
                "Tags: {}",
                note.tags
                    .iter()
                    .map(|t| format!("#{}", t))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        println!(
            "Links: {} outgoing, {} incoming",
            note.wikilink_count, note.backlink_count
        );
        println!("───────────────────────────");
        println!("{}", note.content);
    }

    Ok(())
}

/// Handle `notes list`
pub fn handle_list(db: &Database, limit: usize, tag: Option<String>, json: bool) -> AppResult<()> {
    let notes = db.list_notes(&NoteListQuery {
        limit,
        offset: 0,
        sort: SortOrder::UpdatedDesc,
        tag,
    })?;

    if json {
        println!("{}", serde_json::to_string_pretty(&notes)?);
        return Ok(());
    }

    if notes.is_empty() {
        println!("No notes found.");
        return Ok(());
    }

    println!("{:<38} {:<30} {:<12} TAGS", "ID", "TITLE", "UPDATED");
    println!("{}", "─".repeat(90));
    for note in &notes {
        println!(
            "{:<38} {:<30} {:<12} {}",
            &note.id[..8],
            truncate(&note.title, 28),
            note.updated_at.format("%Y-%m-%d"),
            note.tag_count,
        );
    }
    println!("\n{} notes shown", notes.len());

    Ok(())
}

/// Handle `notes search`
pub fn handle_search(db: &Database, query: String, limit: usize, json: bool) -> AppResult<()> {
    let results = with_spinner(
        "Searching...",
        || {
            db.search_notes(&SearchQuery {
                q: query.clone(),
                limit,
                offset: 0,
            })
        },
        |r| format!("Found {} results", r.len()),
    )?;

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }

    if results.is_empty() {
        println!("No results for \"{}\"", query);
        return Ok(());
    }

    println!("Search results for \"{}\":\n", query);
    for (i, note) in results.iter().enumerate() {
        println!("  {}. {} ({})", i + 1, note.title, &note.id[..8]);
        println!("     {}", truncate(&note.preview, 80));
        println!();
    }

    Ok(())
}

/// Handle `notes graph`
pub fn handle_graph(
    db: &Database,
    format: String,
    center: Option<String>,
    depth: usize,
) -> AppResult<()> {
    let links = db.get_all_links()?;
    let notes = db.list_notes(&NoteListQuery {
        limit: 10000,
        offset: 0,
        sort: SortOrder::UpdatedDesc,
        tag: None,
    })?;

    let mut titles: HashMap<String, String> = HashMap::new();
    let mut tag_counts: HashMap<String, usize> = HashMap::new();
    for note in &notes {
        titles.insert(note.id.clone(), note.title.clone());
        tag_counts.insert(note.id.clone(), note.tag_count);
    }

    let kg = KnowledgeGraph::from_links(&links, &titles, &tag_counts);

    let graph_data = if let Some(center_id) = center {
        kg.export_subgraph(&center_id, depth)
    } else {
        kg.export()
    };

    match format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&graph_data)?);
        }
        "dot" => {
            println!("digraph knowledge_graph {{");
            println!("  rankdir=LR;");
            println!("  node [shape=box, style=rounded];");
            for node in &graph_data.nodes {
                println!(
                    "  \"{}\" [label=\"{}\"];",
                    node.id,
                    node.title.replace('"', "\\\"")
                );
            }
            for edge in &graph_data.edges {
                println!(
                    "  \"{}\" -> \"{}\" [label=\"{}\"];",
                    edge.source, edge.target, edge.link_type
                );
            }
            println!("}}");
        }
        _ => {
            // Text format
            println!("Knowledge Graph");
            println!("═══════════════");
            println!(
                "Nodes: {} | Edges: {} | Orphans: {}",
                graph_data.stats.total_notes,
                graph_data.stats.total_links,
                graph_data.stats.orphan_notes
            );
            if let Some(most) = &graph_data.stats.most_linked {
                println!("Most linked: {}", most);
            }
            println!();

            for edge in &graph_data.edges {
                let src_title = titles.get(&edge.source).cloned().unwrap_or_default();
                let tgt_title = titles.get(&edge.target).cloned().unwrap_or_default();
                println!("  {} ──[{}]──> {}", src_title, edge.link_type, tgt_title);
            }
        }
    }

    Ok(())
}

/// Handle `notes stats`
pub fn handle_stats(db: &Database) -> AppResult<()> {
    let stats = db.get_stats()?;
    println!("Database Statistics");
    println!("═══════════════════");
    println!("  Notes:        {}", stats.total_notes);
    println!("  Links:        {}", stats.total_links);
    println!("  Tags:         {}", stats.total_tags);
    println!("  Orphan notes: {}", stats.orphan_notes);
    if let Some(most) = &stats.most_linked {
        println!("  Most linked:  {}", most);
    }
    Ok(())
}

/// Handle `notes import`
pub fn handle_import(db: &Database, path: String, recursive: bool) -> AppResult<()> {
    let dir = Path::new(&path);
    if !dir.is_dir() {
        return Err(crate::errors::AppError::BadRequest(format!(
            "{} is not a directory",
            path
        )));
    }

    let mut count = 0;
    import_dir(db, dir, recursive, &mut count)?;
    println!("Imported {} notes from {}", count, path);
    Ok(())
}

fn import_dir(db: &Database, dir: &Path, recursive: bool, count: &mut usize) -> AppResult<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() && recursive {
            import_dir(db, &path, recursive, count)?;
        } else if path.extension().is_some_and(|ext| ext == "md") {
            let content = std::fs::read_to_string(&path)?;
            let title = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string();

            let mut tags = parser::extract_tags(&content);
            if let Some((fm, _)) = parser::parse_frontmatter(&content) {
                for tag in fm.tags {
                    if !tags.contains(&tag) {
                        tags.push(tag);
                    }
                }
            }

            db.create_note(CreateNoteRequest {
                title,
                content,
                tags,
            })?;
            *count += 1;
        }
    }
    Ok(())
}

/// Handle `notes export`
pub fn handle_link(
    db: &Database,
    source: String,
    target: String,
    link_type: String,
) -> AppResult<()> {
    // Resolve source: try ID first, then title
    let src_note = match db.get_note(&source) {
        Ok(n) => n,
        Err(_) => db
            .get_note_by_title(&source)?
            .ok_or_else(|| crate::errors::AppError::NoteNotFound(source.clone()))?,
    };

    // Resolve target: try ID first, then title
    let tgt_note = match db.get_note(&target) {
        Ok(n) => n,
        Err(_) => db
            .get_note_by_title(&target)?
            .ok_or_else(|| crate::errors::AppError::NoteNotFound(target.clone()))?,
    };

    let lt = LinkType::parse(&link_type);
    db.create_link(&src_note.id, &tgt_note.id, lt)?;

    println!(
        "Linked: {} --[{}]--> {}",
        src_note.title, link_type, tgt_note.title
    );
    Ok(())
}

pub fn handle_export(db: &Database, path: String, frontmatter: bool) -> AppResult<()> {
    std::fs::create_dir_all(&path)?;

    let notes = db.list_notes(&NoteListQuery {
        limit: 100000,
        offset: 0,
        sort: SortOrder::TitleAsc,
        tag: None,
    })?;

    let mut count = 0;
    for summary in &notes {
        let note = db.get_note(&summary.id)?;
        let filename = format!("{}.md", sanitize_filename(&note.title));
        let filepath = Path::new(&path).join(&filename);

        let content = if frontmatter {
            let tags_str = note
                .tags
                .iter()
                .map(|t| format!("\"{}\"", t))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "---\ntags: [{}]\nid: {}\ncreated: {}\nupdated: {}\n---\n{}",
                tags_str,
                note.id,
                note.created_at.to_rfc3339(),
                note.updated_at.to_rfc3339(),
                note.content
            )
        } else {
            note.content.clone()
        };

        std::fs::write(&filepath, content)?;
        count += 1;
    }

    println!("Exported {} notes to {}", count, path);
    Ok(())
}

/// Handle `smriti cascade <id>` — inspect Benna-Fusi cascade synapse state.
///
/// Resolves `id` as a note id first, then falls back to title — same UX
/// pattern as `handle_read`. Calls into `features::cascade::explain` which
/// brings the on-disk state up to "now" before reading out.
pub fn handle_cascade(db: &Database, id: String, json: bool) -> AppResult<()> {
    use crate::features::cascade::{explain, CascadeConfig};

    // Resolve id → note id (id-or-title), so users can pass either.
    let note_id = match db.get_note(&id) {
        Ok(n) => n.id,
        Err(_) => match db.get_note_by_title(&id)? {
            Some(n) => n.id,
            None => return Err(crate::errors::AppError::NoteNotFound(id)),
        },
    };

    let config = CascadeConfig::default();
    let payload = db.execute(|conn| explain(conn, &note_id, &config))?;

    if json {
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    println!("━━━ cascade synapse state ━━━");
    println!("note:       {}", payload.note_id);
    println!("config:     {}", payload.config_summary);
    println!(
        "last update: {}",
        payload.last_updated.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!("salience:   {:.6}", payload.salience);
    println!();
    println!("  level    capacity (s)    timescale     u_k");
    println!("  -----    ------------    ----------    --------");
    for (k, ((u, c), tau)) in payload
        .levels
        .iter()
        .zip(payload.capacities_seconds.iter())
        .zip(payload.timescales_seconds.iter())
        .enumerate()
    {
        println!(
            "  u_{:<3}    {:>12.0}    {:>10}    {:>8.6}",
            k,
            c,
            format_duration_seconds(*tau),
            u,
        );
    }
    Ok(())
}

/// Pretty-print a duration in seconds at the most appropriate unit.
fn format_duration_seconds(seconds: f64) -> String {
    if seconds < 60.0 {
        format!("{:.1}s", seconds)
    } else if seconds < 3_600.0 {
        format!("{:.1}m", seconds / 60.0)
    } else if seconds < 86_400.0 {
        format!("{:.1}h", seconds / 3_600.0)
    } else {
        format!("{:.1}d", seconds / 86_400.0)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", crate::safe_truncate(s, max.saturating_sub(3)))
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// Handle `smriti init` — scaffold a fresh database and print MCP config.
pub fn handle_init(db_path: &str) -> AppResult<()> {
    use std::path::Path;

    let path = Path::new(db_path);
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    // Check if DB already exists
    if abs_path.exists() {
        return Err(crate::errors::AppError::BadRequest(format!(
            "Database already exists at {}. Delete it or use a different path.",
            abs_path.display()
        )));
    }

    // Create parent directory if needed
    if let Some(parent) = abs_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Initialize the database
    let _db = Database::new(db_path)?;

    println!("\n✓ Initialized Smriti database at: {}\n", abs_path.display());

    // Print MCP configuration block
    println!("Add this to your Claude Desktop config (claude_desktop_config.json):\n");
    println!("{{");
    println!("  \"mcpServers\": {{");
    println!("    \"smriti\": {{");
    println!("      \"command\": \"smriti\",");
    println!("      \"args\": [\"mcp\", \"--db\", \"{}\"]", abs_path.display());
    println!("    }}");
    println!("  }}");
    println!("}}\n");

    println!("Quick start:");
    println!("  smriti create \"My First Note\" -c \"Content here\" -t example");
    println!("  smriti search \"first\"");
    println!("  smriti serve\n");

    Ok(())
}

/// Explain WikiSkill provenance for a schema note.
fn explain_schema_provenance(db: &Database, schema_id: &str, json: bool) -> AppResult<()> {
    #[derive(serde::Serialize)]
    struct SchemaProvenance {
        schema_id: String,
        schema_title: String,
        source_episodes: Vec<SourceEpisode>,
        formation_rationale: Option<String>,
    }

    #[derive(serde::Serialize)]
    struct SourceEpisode {
        episode_id: String,
        episode_title: String,
        similarity_score: f32,
    }

    // Fetch schema title
    let schema_title: String = db.execute(|conn| {
        conn.query_row(
            "SELECT title FROM notes WHERE id = ?1",
            rusqlite::params![schema_id],
            |r| r.get(0),
        )
        .map_err(|e| e.into())
    })?;

    // Fetch source episodes from schema_sources
    let sources: Vec<SourceEpisode> = db.execute(|conn| {
        let mut stmt = conn.prepare(
            "SELECT ss.source_note_id, n.title, ss.similarity_score
             FROM schema_sources ss
             JOIN notes n ON n.id = ss.source_note_id
             WHERE ss.schema_id = ?1
             ORDER BY ss.similarity_score DESC"
        )?;
        let rows = stmt
            .query_map(rusqlite::params![schema_id], |r| {
                Ok(SourceEpisode {
                    episode_id: r.get(0)?,
                    episode_title: r.get(1)?,
                    similarity_score: r.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })?;

    // Fetch formation rationale from consolidation_events
    let rationale: Option<String> = db.execute(|conn| {
        use rusqlite::OptionalExtension;
        conn.query_row(
            "SELECT reason FROM consolidation_events
             WHERE note_id IN (SELECT source_note_id FROM schema_sources WHERE schema_id = ?1)
             AND event_type = 'promoted_to_schema'
             ORDER BY created_at DESC
             LIMIT 1",
            rusqlite::params![schema_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.into())
    })?;

    let provenance = SchemaProvenance {
        schema_id: schema_id.to_string(),
        schema_title,
        source_episodes: sources,
        formation_rationale: rationale,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&provenance)?);
    } else {
        println!("WikiSkill Provenance for Schema: {}", provenance.schema_title);
        println!("  Schema ID: {}", provenance.schema_id);
        println!("  Source Episodes ({}):", provenance.source_episodes.len());
        for ep in &provenance.source_episodes {
            println!(
                "    - {} (similarity: {:.3}): {}",
                ep.episode_id, ep.similarity_score, ep.episode_title
            );
        }
        if let Some(reason) = &provenance.formation_rationale {
            println!("  Formation Rationale: {}", reason);
        }
    }

    Ok(())
}

/// Try to create an inference backend for schema formation.
/// Returns None if config missing or backend unavailable (non-fatal).
fn try_create_backend_for_consolidation() -> Option<crate::inference::SharedBackend> {
    use crate::inference::{create_backend, InferenceConfig};
    
    // Try to load config from env or default locations
    let config = InferenceConfig::default();
    
    // Attempt backend creation (may fail if Ollama not running, etc.)
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            match handle.block_on(create_backend(&config)) {
                Ok(backend) => {
                    tracing::info!("Inference backend available for schema formation: {}", backend.name());
                    Some(backend)
                }
                Err(e) => {
                    tracing::debug!("Inference backend unavailable (will fall back to Extractive): {}", e);
                    None
                }
            }
        }
        Err(_) => {
            tracing::debug!("No tokio runtime for inference backend");
            None
        }
    }
}

/// Handle `smriti consolidate` — run CLS-inspired consolidation pass.
pub fn handle_consolidate(
    db: &Database,
    policy_str: &str,
    dry_run: bool,
    explain: Option<String>,
    json: bool,
) -> AppResult<()> {
    use crate::features::consolidation::{
        run_consolidation_pass, explain_score, ConsolidationPolicy, ScoreWeights, Thresholds,
    };

    let policy = match policy_str {
        "conservative" => ConsolidationPolicy::Conservative,
        "standard" => ConsolidationPolicy::Standard,
        "aggressive" => ConsolidationPolicy::Aggressive,
        _ => {
            return Err(crate::errors::AppError::BadRequest(format!(
                "Unknown policy: {}. Use conservative, standard, or aggressive.",
                policy_str
            )))
        }
    };

    // If --explain is given, show score breakdown for episodes or provenance for schemas
    if let Some(note_id) = explain {
        // Check note type
        let node_type: String = db.execute(|conn| {
            conn.query_row(
                "SELECT node_type FROM notes WHERE id = ?1",
                rusqlite::params![&note_id],
                |r| r.get(0),
            )
            .map_err(|e| e.into())
        })?;

        if node_type == "schema" {
            // Show WikiSkill provenance for schema notes
            explain_schema_provenance(db, &note_id, json)?;
        } else {
            // Show consolidation score breakdown for episode notes
            let breakdown = db.execute(|conn| {
                explain_score(conn, &note_id, ScoreWeights::default())
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&breakdown)?);
            } else {
                println!("Consolidation score breakdown for note {}:", breakdown.note_id);
                println!("  Cascade salience:    {:.6}", breakdown.cascade_salience);
                println!("  Degree (link count): {}", breakdown.degree);
                println!("  Context diversity:   {:.3}", breakdown.context_diversity);
                println!();
                println!("  Salience component:  {:.4}  (weight × salience)", breakdown.salience_component);
                println!("  Degree component:    {:.4}  (weight × ln(1+degree))", breakdown.degree_component);
                println!("  Diversity component: {:.4}  (weight × diversity)", breakdown.diversity_component);
                println!();
                println!("  Raw sum:             {:.4}", breakdown.raw_sum);
                println!("  Final score:         {:.4}  (sigmoid(raw_sum))", breakdown.score);
            }
        }
        return Ok(());
    }

    // Try to create an inference backend for Llm mode (optional)
    // Falls back to Extractive if backend unavailable or not configured
    let backend = try_create_backend_for_consolidation();

    // Run full consolidation pass
    let report = db.execute(|conn| {
        run_consolidation_pass(
            conn,
            policy,
            dry_run,
            ScoreWeights::default(),
            Thresholds::default(),
            backend,
        )
    })?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        let mode = if dry_run { "(dry run)" } else { "" };
        println!("Consolidation pass complete {} — policy: {:?}", mode, report.policy);
        println!("  Scanned:  {} episode notes", report.scanned);
        println!("  Promoted: {} schema(s)", report.promoted.len());
        println!("  Flagged:  {} for review", report.flagged.len());
        println!("  Archived: {} past grace", report.archived.len());

        if !report.promoted.is_empty() {
            println!("\nPromoted to schema:");
            for id in &report.promoted {
                if let Some(reason) = report.reasons.get(id) {
                    println!("  {} — {}", &id[..8], reason);
                }
            }
        }

        if !report.flagged.is_empty() && report.flagged.len() <= 10 {
            println!("\nFlagged for review:");
            for id in &report.flagged {
                if let Some(reason) = report.reasons.get(id) {
                    println!("  {} — {}", &id[..8], reason);
                }
            }
        } else if report.flagged.len() > 10 {
            println!("\n{} notes flagged (showing first 10):", report.flagged.len());
            for id in report.flagged.iter().take(10) {
                if let Some(reason) = report.reasons.get(id) {
                    println!("  {} — {}", &id[..8], reason);
                }
            }
        }

        if !report.archived.is_empty() {
            println!("\nArchived (past grace period):");
            for id in &report.archived {
                if let Some(reason) = report.reasons.get(id) {
                    println!("  {} — {}", &id[..8], reason);
                }
            }
        }

        if dry_run {
            println!("\nNo changes persisted (dry run). Use --dry-run=false to commit.");
        }
    }

    Ok(())
}

/// List schema proposals flagged for review (Conservative policy).
pub fn handle_proposals(db: &Database, json: bool) -> AppResult<()> {
    #[derive(serde::Serialize)]
    struct Proposal {
        note_id: String,
        title: String,
        flagged_at: String,
        consolidation_score: f32,
    }

    let proposals: Vec<Proposal> = db.execute(|conn| {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT n.id, n.title, ce.created_at, n.consolidation_score
             FROM consolidation_events ce
             JOIN notes n ON n.id = ce.note_id
             WHERE ce.event_type = 'flagged_for_review'
             AND n.node_type = 'episode'
             AND n.parent_schema_id IS NULL
             ORDER BY ce.created_at DESC
             LIMIT 50"
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Proposal {
                    note_id: r.get(0)?,
                    title: r.get(1)?,
                    flagged_at: r.get(2)?,
                    consolidation_score: r.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })?;

    if json {
        println!("{}", serde_json::to_string_pretty(&proposals)?);
    } else {
        if proposals.is_empty() {
            println!("No schema proposals pending review.");
        } else {
            println!("Schema proposals pending review ({}):", proposals.len());
            for p in &proposals {
                println!(
                    "  {} (score: {:.3}, flagged: {}): {}",
                    p.note_id, p.consolidation_score, p.flagged_at, p.title
                );
            }
        }
    }

    Ok(())
}

/// Approve a flagged schema proposal (Conservative policy).
pub fn handle_approve_proposal(db: &Database, cluster_id: &str) -> AppResult<()> {
    // For now, cluster_id is a note ID from the flagged episode
    // We need to find similar notes and form a schema
    
    // Check that the note is flagged
    let is_flagged: bool = db.execute(|conn| {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM consolidation_events 
             WHERE note_id = ?1 AND event_type = 'flagged_for_review')",
            rusqlite::params![cluster_id],
            |r| r.get(0),
        )
        .map_err(|e| e.into())
    })?;

    if !is_flagged {
        return Err(crate::errors::AppError::BadRequest(format!(
            "Note {} is not flagged for review", cluster_id
        )));
    }

    // TODO: Actually run schema formation on this cluster
    // For now, just log approval
    db.execute(|conn| {
        use chrono::Utc;
        use uuid::Uuid;
        conn.execute(
            "INSERT INTO consolidation_events
             (id, note_id, event_type, score_before, score_after, reason, created_at)
             VALUES (?1, ?2, 'proposal_approved', NULL, NULL, ?3, ?4)",
            rusqlite::params![
                Uuid::new_v4().to_string(),
                cluster_id,
                "approved by human operator",
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    })?;

    println!("✓ Approved proposal for note {}", cluster_id);
    println!("  Note: Schema formation implementation pending");

    Ok(())
}

/// Reject a flagged schema proposal (Conservative policy).
pub fn handle_reject_proposal(db: &Database, cluster_id: &str, reason: &str) -> AppResult<()> {
    // Check that the note is flagged
    let is_flagged: bool = db.execute(|conn| {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM consolidation_events 
             WHERE note_id = ?1 AND event_type = 'flagged_for_review')",
            rusqlite::params![cluster_id],
            |r| r.get(0),
        )
        .map_err(|e| e.into())
    })?;

    if !is_flagged {
        return Err(crate::errors::AppError::BadRequest(format!(
            "Note {} is not flagged for review", cluster_id
        )));
    }

    // Log rejection (rollback without affecting already-committed schemas)
    db.execute(|conn| {
        use chrono::Utc;
        use uuid::Uuid;
        conn.execute(
            "INSERT INTO consolidation_events
             (id, note_id, event_type, score_before, score_after, reason, created_at)
             VALUES (?1, ?2, 'proposal_rejected', NULL, NULL, ?3, ?4)",
            rusqlite::params![
                Uuid::new_v4().to_string(),
                cluster_id,
                format!("rejected by human: {}", reason),
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    })?;

    println!("✓ Rejected proposal for note {}", cluster_id);
    println!("  Reason: {}", reason);

    Ok(())
}
