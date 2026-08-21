import { useState, useRef, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import client from '../api/client'
import type { NoteSummary } from '../api/types'
import NoteCard from '../components/NoteCard'
import TagChip from '../components/TagChip'
import Skeleton from '../components/Skeleton'
import Toast from '../components/Toast'
import { useToast } from '../hooks/useToast'

const PAGE_SIZE = 20

const QUICK_TAGS = ['client', 'project', 'decision', 'sop', 'research', 'idea', 'daily']

function formatTodayLong(): string {
  return new Date().toLocaleDateString('en-US', {
    weekday: 'long',
    month: 'long',
    day: 'numeric',
  })
}

export default function Dashboard() {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const { toasts, showToast, dismissToast } = useToast()

  // Today bar state
  const [focus, setFocus] = useState('')
  const [focusSaving, setFocusSaving] = useState(false)
  const focusTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Quick capture state
  const [captureOpen, setCaptureOpen] = useState(false)
  const [newTitle, setNewTitle] = useState('')
  const [newContent, setNewContent] = useState('')
  const [selectedTags, setSelectedTags] = useState<string[]>([])
  const [creating, setCreating] = useState(false)

  // Pagination
  const [offset, setOffset] = useState(0)
  const [allNotes, setAllNotes] = useState<NoteSummary[]>([])

  // Fetch stats
  const { data: stats } = useQuery({
    queryKey: ['stats'],
    queryFn: () => client.getStats(),
  })

  // Fetch current focus from KV
  useQuery({
    queryKey: ['kv', 'current_focus'],
    queryFn: async () => {
      try {
        const entry = await client.getKV('current_focus')
        if (typeof entry.value === 'string') {
          setFocus(entry.value)
        }
        return entry
      } catch {
        return null
      }
    },
    staleTime: 60_000,
  })

  // Fetch notes page
  const { data: notesPage, isLoading: notesLoading } = useQuery({
    queryKey: ['notes', offset],
    queryFn: () => client.listNotes({ limit: PAGE_SIZE, offset }),
  })

  useEffect(() => {
    if (notesPage) {
      if (offset === 0) {
        setAllNotes(notesPage)
      } else {
        setAllNotes(prev => {
          const existingIds = new Set(prev.map(n => n.id))
          const newOnes = notesPage.filter(n => !existingIds.has(n.id))
          return [...prev, ...newOnes]
        })
      }
    }
  }, [notesPage, offset])

  // Debounced focus save
  function handleFocusChange(val: string) {
    setFocus(val)
    if (focusTimeoutRef.current) clearTimeout(focusTimeoutRef.current)
    setFocusSaving(true)
    focusTimeoutRef.current = setTimeout(async () => {
      try {
        await client.setKV('current_focus', val)
      } catch {
        // silently fail
      } finally {
        setFocusSaving(false)
      }
    }, 800)
  }

  function toggleTag(tag: string) {
    setSelectedTags(prev =>
      prev.includes(tag) ? prev.filter(t => t !== tag) : [...prev, tag]
    )
  }

  async function handleCreateNote(e: React.FormEvent) {
    e.preventDefault()
    if (!newTitle.trim()) return
    setCreating(true)
    try {
      await client.createNote({
        title: newTitle.trim(),
        content: newContent,
        tags: selectedTags,
      })
      showToast('Note created', 'success')
      setNewTitle('')
      setNewContent('')
      setSelectedTags([])
      setCaptureOpen(false)
      setOffset(0)
      void queryClient.invalidateQueries({ queryKey: ['notes'] })
      void queryClient.invalidateQueries({ queryKey: ['stats'] })
    } catch (err) {
      showToast(err instanceof Error ? err.message : 'Failed to create note', 'error')
    } finally {
      setCreating(false)
    }
  }

  const hasMore = notesPage !== undefined && notesPage.length === PAGE_SIZE

  return (
    <div style={{ padding: '32px 32px 48px', maxWidth: '1080px' }}>

      {/* Today bar */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '24px',
          marginBottom: '32px',
          flexWrap: 'wrap',
        }}
      >
        <div style={{ flex: '0 0 auto' }}>
          <div style={{ fontSize: '12px', fontFamily: 'var(--font-mono)', color: 'var(--text3)', marginBottom: '4px' }}>
            {formatTodayLong()}
          </div>
          {stats && (
            <div style={{ fontSize: '12px', fontFamily: 'var(--font-mono)', color: 'var(--text3)' }}>
              {stats.note_count} notes · {stats.edge_count} edges
            </div>
          )}
        </div>

        <div style={{ flex: 1, minWidth: '200px', position: 'relative' }}>
          <input
            value={focus}
            onChange={(e) => handleFocusChange(e.target.value)}
            placeholder="What's your focus today?"
            aria-label="Current focus"
            style={{
              width: '100%',
              background: 'transparent',
              border: 'none',
              borderBottom: '1px solid var(--border)',
              borderRadius: 0,
              color: 'var(--accent)',
              fontFamily: 'var(--font-serif)',
              fontSize: '18px',
              fontStyle: focus ? 'normal' : 'italic',
              padding: '4px 0',
            }}
          />
          {focusSaving && (
            <span
              style={{
                position: 'absolute',
                right: 0,
                top: '50%',
                transform: 'translateY(-50%)',
                fontSize: '10px',
                color: 'var(--text3)',
                fontFamily: 'var(--font-mono)',
              }}
            >
              saving…
            </span>
          )}
        </div>
      </div>

      {/* Quick capture */}
      <div style={{ marginBottom: '32px' }}>
        {!captureOpen ? (
          <button
            onClick={() => setCaptureOpen(true)}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '8px',
              background: 'var(--surface)',
              border: '1px dashed var(--border)',
              borderRadius: 'var(--radius-md)',
              color: 'var(--text3)',
              padding: '12px 20px',
              fontSize: '13px',
              cursor: 'pointer',
              transition: 'border-color 150ms, color 150ms',
              width: '100%',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.borderColor = 'var(--accent)'
              e.currentTarget.style.color = 'var(--accent)'
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.borderColor = 'var(--border)'
              e.currentTarget.style.color = 'var(--text3)'
            }}
          >
            <span style={{ fontSize: '16px' }}>＋</span>
            New note
          </button>
        ) : (
          <form
            onSubmit={handleCreateNote}
            style={{
              background: 'var(--surface)',
              border: '1px solid var(--border)',
              borderRadius: 'var(--radius-md)',
              padding: '20px',
              display: 'flex',
              flexDirection: 'column',
              gap: '12px',
            }}
          >
            <input
              autoFocus
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              placeholder="Note title"
              required
              aria-label="Note title"
              style={{ width: '100%', fontSize: '15px' }}
            />
            <textarea
              value={newContent}
              onChange={(e) => setNewContent(e.target.value)}
              placeholder="Content… (markdown supported)"
              rows={4}
              aria-label="Note content"
              style={{ width: '100%', resize: 'vertical' }}
            />

            {/* Tag picker */}
            <div>
              <div style={{ fontSize: '11px', color: 'var(--text3)', marginBottom: '6px', fontFamily: 'var(--font-mono)' }}>
                Tags
              </div>
              <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
                {QUICK_TAGS.map(tag => (
                  <TagChip
                    key={tag}
                    tag={tag}
                    active={selectedTags.includes(tag)}
                    onClick={() => toggleTag(tag)}
                  />
                ))}
              </div>
            </div>

            <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
              <button
                type="button"
                onClick={() => {
                  setCaptureOpen(false)
                  setNewTitle('')
                  setNewContent('')
                  setSelectedTags([])
                }}
              >
                Cancel
              </button>
              <button type="submit" className="primary" disabled={creating || !newTitle.trim()}>
                {creating ? 'Creating…' : 'Create note'}
              </button>
            </div>
          </form>
        )}
      </div>

      {/* Recent notes */}
      <div>
        <div style={{ fontSize: '11px', fontFamily: 'var(--font-mono)', color: 'var(--text3)', marginBottom: '16px', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
          Recent notes
        </div>

        {notesLoading && offset === 0 ? (
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fill, minmax(300px, 1fr))',
              gap: '16px',
            }}
          >
            {Array.from({ length: 4 }).map((_, i) => (
              <div
                key={i}
                style={{
                  background: 'var(--surface)',
                  border: '1px solid var(--border)',
                  borderRadius: 'var(--radius-md)',
                  padding: '16px',
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '10px',
                }}
              >
                <Skeleton height={20} width="70%" />
                <Skeleton height={13} />
                <Skeleton height={13} width="85%" />
                <div style={{ display: 'flex', gap: '6px' }}>
                  <Skeleton height={20} width={50} borderRadius={10} />
                  <Skeleton height={20} width={60} borderRadius={10} />
                </div>
              </div>
            ))}
          </div>
        ) : allNotes.length === 0 ? (
          <div
            style={{
              textAlign: 'center',
              padding: '60px 24px',
              color: 'var(--text3)',
              fontFamily: 'var(--font-ui)',
            }}
          >
            <div style={{ fontSize: '32px', marginBottom: '12px', opacity: 0.4 }}>◈</div>
            <div>No notes yet. Create your first note above.</div>
          </div>
        ) : (
          <>
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fill, minmax(300px, 1fr))',
                gap: '16px',
              }}
            >
              {allNotes.map(note => (
                <NoteCard
                  key={note.id}
                  note={note}
                  onClick={() => navigate(`/notes/${note.id}`)}
                />
              ))}
            </div>

            {hasMore && (
              <div style={{ textAlign: 'center', marginTop: '24px' }}>
                <button
                  onClick={() => setOffset(prev => prev + PAGE_SIZE)}
                  disabled={notesLoading}
                  style={{ minWidth: '120px' }}
                >
                  {notesLoading ? 'Loading…' : 'Load more'}
                </button>
              </div>
            )}
          </>
        )}
      </div>

      <Toast toasts={toasts} onDismiss={dismissToast} />
    </div>
  )
}
