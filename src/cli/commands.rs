use clap::{Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser)]
#[command(
    name = "smriti",
    version,
    about = "smriti — memory layer for your AI agents",
    long_about = "Smriti (Sanskrit: memory) — Self-hosted knowledge store built in Rust for agentic AI.\n\
                   Features: MCP server, agent memory, knowledge graph, wiki-links, full-text search, sync.",
    after_help = "\x1b[1mExamples:\x1b[0m\n  \
        smriti create \"Meeting Notes\" -c \"Discussed Q2 roadmap\" -t client,project\n  \
        smriti new                         # interactive guided note creation\n  \
        smriti search \"roadmap\" --limit 5\n  \
        smriti graph --format dot | dot -Tsvg -o graph.svg\n  \
        smriti serve --port 8080\n  \
        smriti completions zsh > ~/.zsh/completions/_smriti",
    styles = styles(),
)]
pub struct Cli {
    /// Path to the database file
    #[arg(long, default_value = "notes.db", global = true)]
    pub db: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    // ── CAPTURE ──────────────────────────────────────────
    /// Create a new note
    #[command(display_order = 1)]
    Create {
        /// Note title
        title: String,

        /// Note content (markdown). Special characters and -- are safe inside quotes.
        #[arg(long, short, allow_hyphen_values = true)]
        content: Option<String>,

        /// Read content from a file
        #[arg(long)]
        file: Option<String>,

        /// Tags: comma-separated (-t client,project) or repeated (--tags client --tags project)
        #[arg(long, short, value_delimiter = ',', num_args = 1..)]
        tags: Option<Vec<String>>,
    },

    /// Create a typed link between two existing notes
    ///
    /// Link types: wikilink, semantic, causal, temporal,
    ///             treats, contraindicts, interacts, indicates, causes
    #[command(
        display_order = 2,
        after_help = "\x1b[1mExamples:\x1b[0m\n  \
            smriti link \"Lisinopril\" \"Hypertension\" --type treats\n  \
            smriti link \"Aspirin\" \"Warfarin\" --type interacts\n  \
            smriti link \"Chest Pain\" \"Myocardial Infarction\" --type indicates\n  \
            smriti link \"Hypertension\" \"Stroke\" --type causes"
    )]
    Link {
        /// Source note: ID or exact title
        source: String,

        /// Target note: ID or exact title
        target: String,

        /// Link type (default: wikilink)
        #[arg(long, short = 'k', default_value = "wikilink")]
        r#type: String,
    },

    /// Interactive guided note creation
    #[cfg(feature = "interactive")]
    #[command(display_order = 3, alias = "n")]
    New,

    // ── SEARCH ───────────────────────────────────────────
    /// Read a note by ID or title
    #[command(display_order = 10)]
    Read {
        /// Note ID or title
        id: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// List all notes
    #[command(display_order = 11)]
    List {
        /// Maximum number of notes to show
        #[arg(long, short, default_value = "20")]
        limit: usize,

        /// Filter by tag
        #[arg(long, short)]
        tag: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Search notes using full-text search
    #[command(display_order = 12)]
    Search {
        /// Search query
        query: String,

        /// Maximum results
        #[arg(long, short, default_value = "10")]
        limit: usize,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    // ── GRAPH ────────────────────────────────────────────
    /// Show the knowledge graph
    #[command(display_order = 20)]
    Graph {
        /// Output format: json, dot, or text
        #[arg(long, short, default_value = "text")]
        format: String,

        /// Center on a specific note ID
        #[arg(long)]
        center: Option<String>,

        /// Depth for subgraph (with --center)
        #[arg(long, default_value = "2")]
        depth: usize,
    },

    /// Show database statistics
    #[command(display_order = 21)]
    Stats,

    // ── SERVER ───────────────────────────────────────────
    /// Start the REST API server
    #[command(display_order = 30)]
    Serve {
        /// Host to bind to
        #[arg(long, default_value = "0.0.0.0")]
        host: String,

        /// Port to listen on
        #[arg(long, short, default_value = "3000")]
        port: u16,
    },

    /// Start the MCP server (JSON-RPC over stdio)
    #[command(display_order = 31)]
    Mcp,

    /// Initialize a new Smriti database
    ///
    /// Creates a fresh database and prints the MCP configuration for Claude Desktop.
    #[command(
        display_order = 32,
        after_help = "\x1b[1mExample:\x1b[0m\n  \
            smriti init\n  \
            smriti init --db /path/to/custom.db"
    )]
    Init,

    /// Generate shell completion scripts
    #[command(
        display_order = 33,
        after_help = "Install completions:\n  \
            bash:       smriti completions bash > ~/.bash_completion.d/smriti\n  \
            zsh:        smriti completions zsh  > ~/.zsh/completions/_smriti\n  \
            fish:       smriti completions fish > ~/.config/fish/completions/smriti.fish\n  \
            powershell: smriti completions powershell >> $PROFILE\n  \
            elvish:     smriti completions elvish > ~/.elvish/lib/smriti.elv"
    )]
    Completions {
        /// Target shell
        shell: Shell,
    },

    // ── DATA ─────────────────────────────────────────────
    /// Sync notes with a remote server (Synology WebDAV or custom)
    #[command(display_order = 40)]
    Sync {
        /// Remote URL (e.g., https://nas.local:5006/notes)
        #[arg(long)]
        remote: String,

        /// Sync direction: push, pull, or both
        #[arg(long, default_value = "both")]
        direction: String,
    },

    /// Import notes from a directory of markdown files
    #[command(display_order = 41)]
    Import {
        /// Directory containing .md files
        path: String,

        /// Recursively import subdirectories
        #[arg(long, short)]
        recursive: bool,
    },

    /// Export notes to a directory of markdown files
    #[command(display_order = 42)]
    Export {
        /// Output directory
        path: String,

        /// Include frontmatter with metadata
        #[arg(long)]
        frontmatter: bool,
    },

    // ── INTEGRITY ────────────────────────────────────────
    /// Verify database integrity (referential + provenance + event log)
    #[command(display_order = 50)]
    Verify {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// List pending wiki transactions awaiting review
    #[command(display_order = 51, name = "pending-tx")]
    PendingTx {
        /// Max transactions to show
        #[arg(long, default_value = "50")]
        limit: usize,
    },

    /// Commit a pending wiki transaction by id
    #[command(display_order = 52, name = "commit-tx")]
    CommitTx {
        /// Transaction id
        transaction_id: String,
    },

    /// Reject a pending wiki transaction by id
    #[command(display_order = 53, name = "reject-tx")]
    RejectTx {
        /// Transaction id
        transaction_id: String,

        /// Who rejected it (default: "human")
        #[arg(long, default_value = "human")]
        by: String,

        /// Reason for rejection
        #[arg(long, short)]
        reason: String,
    },

    /// Scan recent notes for contradiction candidates (MemoTime-style scoring)
    #[command(display_order = 54, name = "detect-contradictions")]
    DetectContradictions {
        /// Max notes to scan pairwise
        #[arg(long, default_value = "50")]
        scan_limit: i64,
    },

    /// List open contradiction candidates for review
    #[command(display_order = 55, name = "contradictions")]
    Contradictions {
        /// Max candidates to show
        #[arg(long, default_value = "50")]
        limit: i64,
    },

    /// Inspect Benna-Fusi cascade synapse state for a note (Migration 011)
    ///
    /// Shows per-level synaptic strengths, capacities, timescales, and the
    /// salience readout. Useful for verifying that a note has consolidated
    /// (mass at deep levels) versus is just freshly accessed (mass at u_0).
    ///
    /// Research ref: Benna & Fusi 2016, Nature Neuroscience 19, 1697–1706.
    #[command(
        display_order = 56,
        name = "cascade",
        after_help = "\x1b[1mExamples:\x1b[0m\n  \
            smriti cascade \"Hypertension\"\n  \
            smriti cascade <note_id> --json"
    )]
    Cascade {
        /// Note ID or exact title
        id: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Run memory consolidation pass (CLS-inspired schema formation)
    ///
    /// Scores all episode notes based on cascade salience + degree + context
    /// diversity, then flags or promotes them based on policy and thresholds.
    ///
    /// Conservative (default for healthcare): flag only, never auto-promote.
    /// Standard: promote eligible clusters to extractive schemas.
    /// Aggressive: promote + auto-archive flagged notes past grace period.
    ///
    /// Research refs:
    ///   CLS — McClelland, McNaughton, O'Reilly 1995 Psych Review
    ///   Benna & Fusi 2016 — cascade multi-timescale synapses
    ///   WikiSkill — arXiv:2608.27454 (architecture reference)
    #[command(
        display_order = 57,
        name = "consolidate",
        after_help = "\x1b[1mExamples:\x1b[0m\n  \
            smriti consolidate --dry-run\n  \
            smriti consolidate --policy standard\n  \
            smriti consolidate --explain <note_id>"
    )]
    Consolidate {
        /// Consolidation policy: conservative (default), standard, or aggressive
        #[arg(long, default_value = "conservative")]
        policy: String,

        /// Dry run: compute scores and report, but do not persist changes
        #[arg(long, default_value = "true")]
        dry_run: bool,

        /// Explain the consolidation score for a specific note
        #[arg(long)]
        explain: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// List schema proposals pending human review (Conservative policy)
    #[command(
        display_order = 58,
        name = "proposals",
        after_help = "\x1b[1mExample:\x1b[0m\n  \
            smriti proposals"
    )]
    Proposals {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Approve a flagged schema proposal (Conservative policy)
    #[command(
        display_order = 59,
        name = "approve",
        after_help = "\x1b[1mExample:\x1b[0m\n  \
            smriti approve <cluster_id>"
    )]
    ApproveProposal {
        /// Cluster ID or note ID from the flagged proposal
        cluster_id: String,
    },

    /// Reject a flagged schema proposal (Conservative policy)
    #[command(
        display_order = 60,
        name = "reject",
        after_help = "\x1b[1mExample:\x1b[0m\n  \
            smriti reject <cluster_id> --reason \"Not a meaningful pattern\""
    )]
    RejectProposal {
        /// Cluster ID or note ID from the flagged proposal
        cluster_id: String,

        /// Reason for rejection
        #[arg(long, short)]
        reason: String,
    },
}

/// Custom styles for colorized --help output.
fn styles() -> clap::builder::Styles {
    use clap::builder::styling::{AnsiColor, Color, Style};

    clap::builder::Styles::styled()
        .usage(
            Style::new()
                .bold()
                .underline()
                .fg_color(Some(Color::Ansi(AnsiColor::Yellow))),
        )
        .header(
            Style::new()
                .bold()
                .underline()
                .fg_color(Some(Color::Ansi(AnsiColor::Yellow))),
        )
        .literal(Style::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan))))
        .placeholder(Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green))))
        .valid(
            Style::new()
                .fg_color(Some(Color::Ansi(AnsiColor::Cyan)))
                .bold(),
        )
}
