//! Smriti benchmark suite — [QUICK-WIN] latency/throughput only, NOT recall quality.
//!
//! Run: cargo bench
//! Groups: insert, fts5_search, graph_ops, memory_kv

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use std::collections::HashMap;

use smriti::graph::KnowledgeGraph;
use smriti::models::*;
use smriti::storage::Database;

// ─── Helpers ───────────────────────────────────────────────────────

fn mem_db() -> Database {
    Database::new(":memory:").expect("in-memory DB")
}

fn seed_notes(db: &Database, n: usize) {
    for i in 0..n {
        db.create_note(CreateNoteRequest {
            title: format!("Note {i}"),
            content: format!(
                "This is the content of note {i}. It discusses topics like Rust, \
                 knowledge graphs, and agent memory systems. Keywords: alpha beta gamma {i}."
            ),
            tags: vec![format!("tag{}", i % 10)],
        })
        .expect("seed note");
    }
}

fn seed_notes_with_links(db: &Database, n: usize) {
    seed_notes(db, n);
    let notes = db
        .list_notes(&NoteListQuery {
            limit: n,
            offset: 0,
            sort: SortOrder::CreatedDesc,
            tag: None,
        })
        .expect("list notes");

    // Create a chain of links: 0->1->2->...->n-1
    for pair in notes.windows(2) {
        let _ = db.create_link(&pair[0].id, &pair[1].id, LinkType::WikiLink);
    }
    // Add some cross-links for graph depth
    if notes.len() > 10 {
        for i in (0..notes.len()).step_by(10) {
            let target = (i + 5) % notes.len();
            let _ = db.create_link(&notes[i].id, &notes[target].id, LinkType::WikiLink);
        }
    }
}

// ─── insert group ──────────────────────────────────────────────────

fn bench_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert");

    group.bench_function("[QUICK-WIN] insert/1", |b| {
        b.iter_batched(
            mem_db,
            |db| {
                db.create_note(CreateNoteRequest {
                    title: "Single Note".into(),
                    content: "Content for a single note with some text.".into(),
                    tags: vec!["bench".into()],
                })
                .expect("insert");
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("[QUICK-WIN] insert/100", |b| {
        b.iter_batched(mem_db, |db| seed_notes(&db, 100), BatchSize::SmallInput);
    });

    group.bench_function("[QUICK-WIN] insert/1000", |b| {
        b.iter_batched(mem_db, |db| seed_notes(&db, 1000), BatchSize::SmallInput);
    });

    group.finish();
}

// ─── fts5_search group ─────────────────────────────────────────────

fn bench_fts5_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("fts5_search");

    // 1k notes
    let db_1k = mem_db();
    seed_notes(&db_1k, 1_000);

    group.bench_function("[QUICK-WIN] fts5_search/1k_notes", |b| {
        b.iter(|| {
            db_1k
                .search_notes(black_box(&SearchQuery {
                    q: "Rust knowledge".into(),
                    limit: 20,
                    offset: 0,
                }))
                .expect("search");
        });
    });

    // 10k notes
    let db_10k = mem_db();
    seed_notes(&db_10k, 10_000);

    group.bench_function("[QUICK-WIN] fts5_search/10k_notes", |b| {
        b.iter(|| {
            db_10k
                .search_notes(black_box(&SearchQuery {
                    q: "Rust knowledge".into(),
                    limit: 20,
                    offset: 0,
                }))
                .expect("search");
        });
    });

    group.finish();
}

// ─── graph_ops group ───────────────────────────────────────────────

fn bench_graph_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_ops");

    // Prepare data for graph build benchmarks
    let db = mem_db();
    seed_notes_with_links(&db, 1_000);

    let links = db.get_all_links().expect("get links");
    let notes = db
        .list_notes(&NoteListQuery {
            limit: 10_000,
            offset: 0,
            sort: SortOrder::UpdatedDesc,
            tag: None,
        })
        .expect("list notes");

    let mut titles: HashMap<String, String> = HashMap::new();
    let mut tag_counts: HashMap<String, usize> = HashMap::new();
    for note in &notes {
        titles.insert(note.id.clone(), note.title.clone());
        tag_counts.insert(note.id.clone(), note.tag_count);
    }

    group.bench_function("[QUICK-WIN] graph_ops/build_1k", |b| {
        b.iter(|| {
            KnowledgeGraph::from_links(
                black_box(&links),
                black_box(&titles),
                black_box(&tag_counts),
            )
        });
    });

    // BFS benchmarks on a pre-built graph
    let kg = KnowledgeGraph::from_links(&links, &titles, &tag_counts);
    let center_id = &notes[0].id;

    group.bench_function("[QUICK-WIN] graph_ops/bfs_d2", |b| {
        b.iter(|| kg.get_related_notes(black_box(center_id), black_box(2)));
    });

    group.bench_function("[QUICK-WIN] graph_ops/bfs_d3", |b| {
        b.iter(|| kg.get_related_notes(black_box(center_id), black_box(3)));
    });

    group.finish();
}

// ─── memory_kv group ───────────────────────────────────────────────

fn bench_memory_kv(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_kv");

    group.bench_function("[QUICK-WIN] memory_kv/store", |b| {
        b.iter_batched(
            mem_db,
            |db| {
                for i in 0..100 {
                    db.store_memory(
                        "bench-agent",
                        CreateMemoryRequest {
                            namespace: Some("bench".into()),
                            key: format!("key-{i}"),
                            value: serde_json::json!({"data": i}),
                            ttl_seconds: None,
                        },
                    )
                    .expect("store");
                }
            },
            BatchSize::SmallInput,
        );
    });

    // Pre-populate for retrieve benchmarks
    let db = mem_db();
    for i in 0..100 {
        db.store_memory(
            "bench-agent",
            CreateMemoryRequest {
                namespace: Some("bench".into()),
                key: format!("key-{i}"),
                value: serde_json::json!({"data": i}),
                ttl_seconds: None,
            },
        )
        .expect("seed memory");
    }

    group.bench_function("[QUICK-WIN] memory_kv/retrieve_hit", |b| {
        b.iter(|| {
            db.get_memory(
                black_box("bench-agent"),
                black_box("bench"),
                black_box("key-50"),
            )
            .expect("retrieve");
        });
    });

    group.bench_function("[QUICK-WIN] memory_kv/retrieve_miss", |b| {
        b.iter(|| {
            let _ = db.get_memory(
                black_box("bench-agent"),
                black_box("bench"),
                black_box("nonexistent-key"),
            );
        });
    });

    group.finish();
}

// ─── Main ──────────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_insert,
    bench_fts5_search,
    bench_graph_ops,
    bench_memory_kv
);
criterion_main!(benches);
