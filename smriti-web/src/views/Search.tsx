import { useState, useEffect, useRef, useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import client from '../api/client'
import type { SearchMode } from '../api/types'
import TagChip from '../components/TagChip'
import NoteCard from '../components/NoteCard'
import Skeleton from '../components/Skeleton'

const MODES: { value: SearchMode; label: string }[] = [
  { value: 'fts', label: 'FTS' },
  { value: 'semantic', label: 'Semantic' },
  { value: 'hybrid', label: 'Hybrid' },
]


export default function Search() {
  const navigate = useNavigate()
  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

  const [query, setQuery] = useState('')
  const [debouncedQuery, setDebouncedQuery] = useState('')
  const [mode, setMode] = useState<SearchMode>('fts')
  const [cursor, setCursor] = useState(-1)

  // Autofocus on mount
  useEffect(() => {
    inputRef.current?.focus()
  }, [])

  // Debounce
  useEffect(() => {
    const t = setTimeout(() => setDebouncedQuery(query), 200)
    return () => clearTimeout(t)
  }, [query])

  // Reset cursor when results change
  useEffect(() => {
    setCursor(-1)
  }, [debouncedQuery, mode])

  const { data: results, isLoading } = useQuery({
    queryKey: ['search', debouncedQuery, mode],
    queryFn: () => client.search(debouncedQuery, mode),
    enabled: debouncedQuery.length > 0,
  })

  // Recent notes shown when search box is empty
  const { data: recentNotes, isLoading: recentLoading } = useQuery({
    queryKey: ['notes', 0],
    queryFn: () => client.listNotes({ limit: 20 }),
    enabled: debouncedQuery.length === 0,
  })

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    const len = results?.length ?? 0
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setCursor(c => Math.min(c + 1, len - 1))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setCursor(c => Math.max(c - 1, -1))
    } else if (e.key === 'Enter') {
      e.preventDefault()
      if (cursor >= 0 && results?.[cursor]) {
        navigate(`/notes/${results[cursor].id}`)
      }
    } else if (e.key === 'Escape') {
      setQuery('')
      setCursor(-1)
    }
  }, [results, cursor, navigate])

  // Scroll selected item into view
  useEffect(() => {
    if (cursor >= 0 && listRef.current) {
      const items = listRef.current.querySelectorAll('[data-result-item]')
      const el = items[cursor] as HTMLElement | undefined
      el?.scrollIntoView({ block: 'nearest' })
    }
  }, [cursor])

  const showResults = debouncedQuery.length > 0

  return (
    <div style={{ padding: '32px 32px 48px', maxWidth: '800px' }}>

      {/* Search input */}
      <div style={{ marginBottom: '20px' }}>
        <div
          style={{
            position: 'relative',
            display: 'flex',
            alignItems: 'center',
          }}
        >
          <span
            aria-hidden="true"
            style={{
              position: 'absolute',
              left: '14px',
              fontSize: '18px',
              color: 'var(--text3)',
              pointerEvents: 'none',
              lineHeight: 1,
            }}
          >
            ⌕
          </span>
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Search your knowledge graph…"
            aria-label="Search notes"
            aria-autocomplete="list"
            aria-expanded={showResults && (results?.length ?? 0) > 0}
            role="combobox"
            style={{
              width: '100%',
              paddingLeft: '44px',
              fontSize: '16px',
              height: '48px',
              borderRadius: 'var(--radius-md)',
            }}
          />
          {query && (
            <button
              onClick={() => { setQuery(''); inputRef.current?.focus() }}
              aria-label="Clear search"
              style={{
                position: 'absolute',
                right: '12px',
                background: 'none',
                border: 'none',
                padding: '4px 8px',
                color: 'var(--text3)',
                cursor: 'pointer',
                fontSize: '16px',
              }}
            >
              ×
            </button>
          )}
        </div>
      </div>

      {/* Mode toggle */}
      <div
        role="group"
        aria-label="Search mode"
        style={{
          display: 'inline-flex',
          background: 'var(--surface)',
          border: '1px solid var(--border)',
          borderRadius: 'var(--radius-md)',
          padding: '3px',
          marginBottom: '24px',
          gap: '2px',
        }}
      >
        {MODES.map(m => (
          <button
            key={m.value}
            onClick={() => setMode(m.value)}
            aria-pressed={mode === m.value}
            style={{
              padding: '5px 14px',
              borderRadius: 'var(--radius-sm)',
              border: 'none',
              background: mode === m.value ? 'var(--amber)' : 'transparent',
              color: mode === m.value ? 'var(--bg)' : 'var(--text2)',
              fontWeight: mode === m.value ? 600 : 400,
              fontSize: '12px',
              cursor: 'pointer',
              transition: 'background 150ms, color 150ms',
            }}
          >
            {m.label}
          </button>
        ))}
      </div>

      {/* Results / states */}
      {!showResults && (
        <>
          <div style={{ fontSize: '11px', fontFamily: 'var(--font-mono)', color: 'var(--text3)', marginBottom: '16px', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
            Recent notes
          </div>
          {recentLoading ? (
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: '12px' }}>
              {Array.from({ length: 6 }).map((_, i) => <Skeleton key={i} height={96} />)}
            </div>
          ) : (recentNotes?.length ?? 0) === 0 ? (
            <div style={{ textAlign: 'center', padding: '60px 24px', color: 'var(--text3)' }}>
              <div style={{ fontSize: '32px', marginBottom: '12px', opacity: 0.4 }}>◈</div>
              <div>No notes yet. Create your first note on the dashboard.</div>
            </div>
          ) : (
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: '12px' }}>
              {recentNotes!.map(note => (
                <NoteCard key={note.id} note={note} onClick={() => navigate(`/notes/${note.id}`)} />
              ))}
            </div>
          )}
        </>
      )}

      {showResults && isLoading && (
        <div
          role="status"
          aria-label="Loading results"
          style={{ display: 'flex', flexDirection: 'column', gap: '1px' }}
        >
          {Array.from({ length: 5 }).map((_, i) => (
            <div
              key={i}
              style={{
                padding: '16px',
                background: 'var(--surface)',
                border: '1px solid var(--border)',
                borderRadius: 'var(--radius-md)',
                marginBottom: '8px',
                display: 'flex',
                flexDirection: 'column',
                gap: '8px',
              }}
            >
              <Skeleton height={18} width="55%" />
              <Skeleton height={13} />
              <Skeleton height={13} width="80%" />
            </div>
          ))}
        </div>
      )}

      {showResults && !isLoading && results?.length === 0 && (
        <div
          style={{
            textAlign: 'center',
            padding: '60px 24px',
            color: 'var(--text3)',
          }}
        >
          <div style={{ fontSize: '32px', marginBottom: '12px', opacity: 0.4 }}>∅</div>
          <div>
            No results for &ldquo;<span style={{ color: 'var(--text2)' }}>{debouncedQuery}</span>&rdquo;
          </div>
          <div style={{ fontSize: '12px', marginTop: '8px', fontFamily: 'var(--font-mono)' }}>
            Try different keywords or switch mode
          </div>
        </div>
      )}

      {showResults && !isLoading && results && results.length > 0 && (
        <div
          ref={listRef}
          role="listbox"
          aria-label="Search results"
          style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}
        >
          {results.map((result, idx) => (
            <div
              key={result.id}
              data-result-item
              role="option"
              aria-selected={cursor === idx}
              tabIndex={0}
              onClick={() => navigate(`/notes/${result.id}`)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') navigate(`/notes/${result.id}`)
              }}
              style={{
                padding: '16px',
                background: cursor === idx ? 'var(--surface2)' : 'var(--surface)',
                border: `1px solid ${cursor === idx ? 'var(--accent)' : 'var(--border)'}`,
                borderRadius: 'var(--radius-md)',
                cursor: 'pointer',
                transition: 'border-color 150ms, background 150ms',
                outline: 'none',
              }}
              onMouseEnter={(e) => {
                setCursor(idx)
                e.currentTarget.style.borderColor = 'var(--accent)'
              }}
              onMouseLeave={(e) => {
                if (cursor !== idx) {
                  e.currentTarget.style.borderColor = 'var(--border)'
                }
              }}
              onFocus={() => setCursor(idx)}
            >
              {/* Title */}
              <div
                style={{
                  fontFamily: 'var(--font-serif)',
                  fontSize: '16px',
                  fontWeight: 600,
                  color: 'var(--text)',
                  marginBottom: '6px',
                }}
              >
                {result.title}
              </div>

              {/* Excerpt */}
              {result.preview && (
                <div
                  style={{
                    fontSize: '13px',
                    color: 'var(--text2)',
                    lineHeight: 1.5,
                    marginBottom: '8px',
                    overflow: 'hidden',
                    display: '-webkit-box',
                    WebkitLineClamp: 2,
                    WebkitBoxOrient: 'vertical',
                  }}
                >
                  {result.preview}
                </div>
              )}

              {/* Footer */}
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
                {result.match_source && (
                  <TagChip tag={result.match_source} />
                )}
                {result.score !== undefined && (
                  <span style={{ fontSize: '11px', color: 'var(--text3)', fontFamily: 'var(--font-mono)' }}>
                    score: {result.score.toFixed(3)}
                  </span>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
