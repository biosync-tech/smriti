export interface Note {
  id: string;
  title: string;
  content: string;
  tags: string[];
  created_at: string;
  updated_at: string;
  backlink_count: number;
  wikilink_count: number;
}

export interface NoteSummary {
  id: string;
  title: string;
  preview: string;
  tags: string[];
  tag_count: number;
  backlink_count: number;
  created_at: string;
  updated_at: string;
}

export interface SearchResult {
  id: string;
  title: string;
  preview: string;
  score?: number;
  match_source?: string;
}

export interface KVEntry {
  id: string;
  agent_id: string;
  namespace: string;
  key: string;
  value: unknown;
  created_at: string;
  updated_at: string;
  ttl_seconds: number | null;
}

export interface WebStats {
  note_count: number;
  edge_count: number;
  kv_count: number;
  db_size_bytes: number;
}

export interface ApiResponse<T> {
  data: T | null;
  error: { code: string; message: string } | null;
}

export type SearchMode = 'fts' | 'semantic' | 'hybrid';

// ── Graph explorer ───────────────────────────────────────────────────────────

export interface FullGraphNode {
  id: string;
  title: string;
  tag_count: number;
  link_count: number;
  created_at: string;
  primary_tag: string | null;
}

export interface FullGraphEdge {
  source: string;
  target: string;
  rel_type: string;
  valid_from?: string;
  valid_until?: string;
}

export interface FullGraphData {
  nodes: FullGraphNode[];
  links: FullGraphEdge[];
  total_notes: number;
  total_links: number;
}
