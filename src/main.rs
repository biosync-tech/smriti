use clap::Parser;
use std::sync::Arc;

use smriti::cli::commands::{Cli, Commands};
use smriti::cli::{completions, handlers};
use smriti::features::smart_links::SmartLinker;
use smriti::mcp::server::McpServer;
use smriti::storage::Database;
use smriti::sync::engine::{SyncDirection, SyncEngine};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();

    // Commands that don't need a database
    match &cli.command {
        Commands::Completions { shell } => {
            completions::generate_completions(*shell);
            return Ok(());
        }
        Commands::Init => {
            handlers::handle_init(&cli.db)?;
            return Ok(());
        }
        _ => {}
    }

    let db = Arc::new(Database::new(&cli.db)?);

    match cli.command {
        Commands::Create {
            title,
            content,
            file,
            tags,
        } => {
            handlers::handle_create(&db, title, content, file, tags)?;
        }

        #[cfg(feature = "interactive")]
        Commands::New => {
            smriti::cli::interactive::interactive_new(&db)?;
        }

        Commands::Link {
            source,
            target,
            r#type,
        } => {
            handlers::handle_link(&db, source, target, r#type)?;
        }

        Commands::Read { id, json } => {
            handlers::handle_read(&db, id, json)?;
        }

        Commands::List { limit, tag, json } => {
            handlers::handle_list(&db, limit, tag, json)?;
        }

        Commands::Search { query, limit, json } => {
            handlers::handle_search(&db, query, limit, json)?;
        }

        Commands::Graph {
            format,
            center,
            depth,
        } => {
            handlers::handle_graph(&db, format, center, depth)?;
        }

        Commands::Stats => {
            handlers::handle_stats(&db)?;

            // Also show smart link suggestions
            println!();
            let suggestions = SmartLinker::find_suggestions(&db, 5)?;
            if !suggestions.is_empty() {
                println!("Suggested Connections:");
                for s in &suggestions {
                    println!(
                        "  {} ↔ {} ({:.0}% - {})",
                        s.source_title,
                        s.target_title,
                        s.confidence * 100.0,
                        s.reason
                    );
                }
            }
        }

        Commands::Serve { host, port } => {
            println!("\n  Smriti v{}\n", env!("CARGO_PKG_VERSION"));
            #[cfg(feature = "webui")]
            println!("  Web UI:  http://{}:{}/", host, port);
            println!("  API:     http://{}:{}/api/v1/", host, port);
            #[cfg(feature = "webui")]
            println!("  MCP:     http://{}:{}/mcp  (HTTP JSON-RPC)", host, port);
            println!();

            smriti::api::server::start_server(db, &host, port, cli.db.clone()).await?;
        }

        Commands::Mcp => {
            let mcp = McpServer::new(db);
            mcp.run()?;
        }

        Commands::Completions { .. } => unreachable!(),
        Commands::Init => unreachable!(),

        Commands::Sync { remote, direction } => {
            let device_id = hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".into());

            let engine = SyncEngine::new(device_id, db.clone());
            let dir = SyncDirection::parse(&direction);

            // Detect if remote is a file path or URL
            if remote.starts_with("http://") || remote.starts_with("https://") {
                println!("WebDAV sync with {} ({})...", remote, direction);
                println!("Note: Set SYNC_USER and SYNC_PASS environment variables for auth");

                let username = std::env::var("SYNC_USER").unwrap_or_else(|_| "admin".into());
                let password = std::env::var("SYNC_PASS").unwrap_or_else(|_| "".into());

                let result = engine
                    .sync_webdav(&remote, &username, &password, dir)
                    .await?;

                println!("Sync complete:");
                println!("  Pushed: {}", result.pushed);
                println!("  Pulled: {}", result.pulled);
                if !result.errors.is_empty() {
                    println!("  Errors:");
                    for err in &result.errors {
                        println!("    - {}", err);
                    }
                }
            } else {
                println!("Filesystem sync with {} ({})...", remote, direction);
                let result = engine.sync_filesystem(&remote, dir)?;
                println!("Sync complete:");
                println!("  Pushed: {}", result.pushed);
                println!("  Pulled: {}", result.pulled);
            }
        }

        Commands::Import { path, recursive } => {
            handlers::handle_import(&db, path, recursive)?;
        }

        Commands::Export { path, frontmatter } => {
            handlers::handle_export(&db, path, frontmatter)?;
        }

        Commands::Verify { json } => {
            let report = db.verify()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                let status = if report.ok {
                    "\x1b[32mOK\x1b[0m"
                } else {
                    "\x1b[31mFAIL\x1b[0m"
                };
                println!("smriti verify: {}", status);
                println!(
                    "  notes={}  links={}  sources={}  claim_spans={}  events={}  grounded_notes={}",
                    report.stats.notes,
                    report.stats.links,
                    report.stats.sources,
                    report.stats.claim_spans,
                    report.stats.events,
                    report.stats.grounded_notes
                );
                if !report.referential_errors.is_empty() {
                    println!(
                        "\nReferential errors ({}):",
                        report.referential_errors.len()
                    );
                    for e in &report.referential_errors {
                        println!("  - {}", e);
                    }
                }
                if !report.provenance_failures.is_empty() {
                    println!(
                        "\nProvenance failures ({}):",
                        report.provenance_failures.len()
                    );
                    for f in &report.provenance_failures {
                        println!(
                            "  - claim_span {} (note {}): stored={:.3} current={:.3} — {}",
                            f.claim_span_id, f.note_id, f.stored_score, f.current_score, f.reason
                        );
                    }
                }
                if !report.event_chain_errors.is_empty() {
                    println!(
                        "\nEvent log chain errors ({}):",
                        report.event_chain_errors.len()
                    );
                    for e in &report.event_chain_errors {
                        println!("  - {}", e);
                    }
                }
                if !report.orphan_notes.is_empty() {
                    println!(
                        "\nOrphan notes (warning only): {}",
                        report.orphan_notes.len()
                    );
                }
            }
        }

        Commands::PendingTx { limit } => {
            let txs = db.list_pending_transactions(limit)?;
            if txs.is_empty() {
                println!("No pending transactions.");
            } else {
                println!("{} pending transaction(s):", txs.len());
                for t in txs {
                    println!(
                        "  {}  agent={}  ops={}  created={}",
                        t.id,
                        t.agent_id,
                        t.operations.len(),
                        t.created_at.to_rfc3339()
                    );
                    if let Some(r) = &t.rationale {
                        println!("    rationale: {}", r);
                    }
                }
            }
        }

        Commands::CommitTx { transaction_id } => {
            let result = db.commit_wiki_transaction(&transaction_id)?;
            println!(
                "Committed {}: notes+{} sources+{} links+{} claims_verified={}",
                result.transaction_id,
                result.created_note_ids.len(),
                result.created_source_ids.len(),
                result.created_link_count,
                result.verified_claim_count
            );
        }

        Commands::RejectTx {
            transaction_id,
            by,
            reason,
        } => {
            db.reject_wiki_transaction(&transaction_id, &by, &reason)?;
            println!("Rejected {}", transaction_id);
        }

        Commands::DetectContradictions { scan_limit } => {
            let cfg = smriti::features::contradiction::ContradictionConfig::default();
            let events = db.detect_contradictions(scan_limit, cfg)?;
            println!("Detected {} candidate contradiction(s).", events.len());
            for e in events {
                println!(
                    "  {:.3}  {} ↔ {}  (sem={:.2} rec={:.2} auth={:.2})",
                    e.combined_score,
                    e.note_id_a,
                    e.note_id_b,
                    e.semantic_score,
                    e.recency_score,
                    e.authority_score
                );
            }
        }

        Commands::Contradictions { limit } => {
            let events = db.list_open_contradictions(limit)?;
            if events.is_empty() {
                println!("No open contradictions.");
            } else {
                println!("{} open contradiction(s):", events.len());
                for e in events {
                    println!(
                        "  {:.3}  {} ↔ {}  detected={}",
                        e.combined_score,
                        e.note_id_a,
                        e.note_id_b,
                        e.detected_at.to_rfc3339()
                    );
                }
            }
        }

        Commands::Cascade { id, json } => {
            handlers::handle_cascade(&db, id, json)?;
        }

        Commands::Consolidate {
            policy,
            dry_run,
            explain,
            json,
        } => {
            handlers::handle_consolidate(&db, &policy, dry_run, explain, json)?;
        }

        Commands::Proposals { json } => {
            handlers::handle_proposals(&db, json)?;
        }

        Commands::ApproveProposal { cluster_id } => {
            handlers::handle_approve_proposal(&db, &cluster_id)?;
        }

        Commands::RejectProposal { cluster_id, reason } => {
            handlers::handle_reject_proposal(&db, &cluster_id, &reason)?;
        }
    }

    Ok(())
}
