import type { Note, NoteSummary, SearchResult, KVEntry, WebStats, SearchMode, FullGraphData } from './types'

const API_BASE = import.meta.env.VITE_API_BASE ?? '/api/v1'

async function request<T>(
  path: string,
  options?: RequestInit,
): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    headers: { 'Content-Type': 'application/json', ...options?.headers },
    ...options,
  })

  // 204 No Content
  if (res.status === 204) {
    return undefined as T
  }

  const json = await res.json()

  if (!res.ok) {
    // Unwrap ApiResponse error envelope if present
    const message: string =
      json?.error?.message ?? json?.message ?? `HTTP ${res.status}`
    throw new Error(message)
  }

  // Unwrap Smriti's ApiResponse<T> envelope: { data: T, error: null }
  if (json && typeof json === 'object' && 'data' in json) {
    if (json.error) throw new Error(json.error.message ?? 'API error')
    return json.data as T
  }

  return json as T
}

function buildQuery(params: Record<string, string | number | undefined>): string {
  const p = new URLSearchParams()
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== '') {
      p.set(k, String(v))
    }
  }
  const s = p.toString()
  return s ? `?${s}` : ''
}

// Raw stats shape from the server (field names may vary)
interface RawStats {
  note_count?: number;
  total_notes?: number;
  edge_count?: number;
  total_links?: number;
  link_count?: number;
  kv_count?: number;
  total_kv?: number;
  db_size_bytes?: number;
  db_size?: number;
}

const client = {
  // Notes
  listNotes(params?: { limit?: number; offset?: number; tag?: string }): Promise<NoteSummary[]> {
    const q = buildQuery({
      limit: params?.limit,
      offset: params?.offset,
      tag: params?.tag,
    })
    return request<NoteSummary[]>(`/notes${q}`)
  },

  createNote(body: { title: string; content: string; tags?: string[] }): Promise<Note> {
    return request<Note>('/notes', {
      method: 'POST',
      body: JSON.stringify(body),
    })
  },

  getNote(id: string): Promise<Note> {
    return request<Note>(`/notes/${encodeURIComponent(id)}`)
  },

  updateNote(id: string, body: { title?: string; content?: string; tags?: string[] }): Promise<Note> {
    return request<Note>(`/notes/${encodeURIComponent(id)}`, {
      method: 'PUT',
      body: JSON.stringify(body),
    })
  },

  getNoteLinks(id: string): Promise<NoteSummary[]> {
    return request<NoteSummary[]>(`/notes/${encodeURIComponent(id)}/links`)
  },

  // Search — falls back to /notes/search if /search returns 404
  async search(q: string, mode: SearchMode): Promise<SearchResult[]> {
    const query = buildQuery({ q, mode })
    try {
      return await request<SearchResult[]>(`/search${query}`)
    } catch (err) {
      // If the /search endpoint doesn't exist, fall back
      if (err instanceof Error && err.message.startsWith('HTTP 404')) {
        const fallbackQuery = buildQuery({ q })
        return request<SearchResult[]>(`/notes/search${fallbackQuery}`)
      }
      throw err
    }
  },

  // KV store
  listKV(prefix?: string): Promise<KVEntry[]> {
    const q = prefix ? buildQuery({ prefix }) : ''
    return request<KVEntry[]>(`/kv${q}`)
  },

  getKV(key: string): Promise<KVEntry> {
    return request<KVEntry>(`/kv/${encodeURIComponent(key)}`)
  },

  setKV(key: string, value: unknown, ttl?: number): Promise<KVEntry> {
    return request<KVEntry>(`/kv/${encodeURIComponent(key)}`, {
      method: 'PUT',
      body: JSON.stringify({ value, ttl_seconds: ttl ?? null }),
    })
  },

  deleteKV(key: string): Promise<void> {
    return request<void>(`/kv/${encodeURIComponent(key)}`, {
      method: 'DELETE',
    })
  },

  // Full graph
  getFullGraph(params?: { limit?: number; q?: string }): Promise<FullGraphData> {
    const q = buildQuery({ limit: params?.limit ?? 200, q: params?.q })
    return request<FullGraphData>(`/graph/full${q}`)
  },

  // Stats
  async getStats(): Promise<WebStats> {
    const raw = await request<RawStats>('/stats')
    return {
      note_count: raw.note_count ?? raw.total_notes ?? 0,
      edge_count: raw.edge_count ?? raw.total_links ?? raw.link_count ?? 0,
      kv_count: raw.kv_count ?? raw.total_kv ?? 0,
      db_size_bytes: raw.db_size_bytes ?? raw.db_size ?? 0,
    }
  },
}

export default client
