import { useState, lazy, Suspense } from 'react'
import { useParams, Link, useNavigate } from 'react-router-dom'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { marked } from 'marked'
import client from '../api/client'
import TagChip from '../components/TagChip'
import Skeleton from '../components/Skeleton'
import Toast from '../components/Toast'
import { useToast } from '../hooks/useToast'

// SmritiEditor is heavy (CodeMirror) — lazy load to keep note reading fast
const SmritiEditor = lazy(() => import('../components/SmritiEditor'))

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    })
  } catch {
    return iso
  }
}

function truncateId(id: string): string {
  return id.length > 8 ? `${id.slice(0, 8)}…` : id
}

export default function NoteDetail() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const { toasts, showToast, dismissToast } = useToast()
  const [idCopied, setIdCopied] = useState(false)
  const [editMode, setEditMode] = useState(false)
  const [editContent, setEditContent] = useState('')

  const { data: note, isLoading, isError, error } = useQuery({
    queryKey: ['note', id],
    queryFn: () => client.getNote(id!),
    enabled: !!id,
  })

  const { data: links, isLoading: linksLoading } = useQuery({
    queryKey: ['note-links', id],
    queryFn: () => client.getNoteLinks(id!),
    enabled: !!id,
  })

  function handleCopyId() {
    if (!note) return
    navigator.clipboard.writeText(note.id).then(() => {
      setIdCopied(true)
      setTimeout(() => setIdCopied(false), 1500)
    }).catch(() => showToast('Failed to copy ID', 'error'))
  }

  if (isError) {
    return (
      <div style={{ padding: '32px', maxWidth: '800px' }}>
        <Link
          to="/"
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: '6px',
            color: 'var(--text3)',
            textDecoration: 'none',
            fontSize: '13px',
            marginBottom: '24px',
            fontFamily: 'var(--font-ui)',
          }}
        >
          ← Back
        </Link>
        <div
          style={{
            background: 'var(--surface)',
            border: '1px solid var(--error)',
            borderRadius: 'var(--radius-md)',
            padding: '32px',
            textAlign: 'center',
          }}
        >
          <div style={{ fontSize: '32px', marginBottom: '12px' }}>⚠</div>
          <div style={{ color: 'var(--error)', marginBottom: '8px', fontWeight: 600 }}>
            Failed to load note
          </div>
          <div style={{ color: 'var(--text3)', fontSize: '13px' }}>
            {error instanceof Error ? error.message : 'Unknown error'}
          </div>
        </div>
      </div>
    )
  }

  if (isLoading || !note) {
    return (
      <div style={{ padding: '32px', maxWidth: '800px' }}>
        <div style={{ marginBottom: '24px' }}>
          <Skeleton height={13} width={80} />
        </div>
        <Skeleton height={36} width="60%" />
        <div style={{ marginTop: '16px', marginBottom: '32px', display: 'flex', gap: '12px' }}>
          <Skeleton height={12} width={140} />
          <Skeleton height={12} width={120} />
        </div>
        {Array.from({ length: 6 }).map((_, i) => (
          <div key={i} style={{ marginBottom: '10px' }}>
            <Skeleton height={14} width={i % 3 === 0 ? '60%' : i % 2 === 0 ? '90%' : '75%'} />
          </div>
        ))}
      </div>
    )
  }

  const htmlContent = marked.parse(note!.content) as string

  function handleEditOpen() {
    setEditContent(note!.content)
    setEditMode(true)
  }

  function handleEditCancel() {
    setEditMode(false)
    setEditContent('')
  }

  function handleLinkClick(title: string) {
    // Navigate to a note by title — search first
    client.search(title, 'fts').then(results => {
      if (results.length > 0) navigate(`/notes/${results[0].id}`)
      else showToast(`Note "${title}" not found`, 'error')
    }).catch(() => showToast('Search failed', 'error'))
  }

  return (
    <div style={{ padding: '32px 32px 64px', maxWidth: '800px' }}>

      {/* Breadcrumb */}
      <div style={{ marginBottom: '24px' }}>
        <Link
          to="/"
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: '6px',
            color: 'var(--text3)',
            textDecoration: 'none',
            fontSize: '13px',
            fontFamily: 'var(--font-ui)',
            transition: 'color 150ms',
          }}
          onMouseEnter={(e) => { e.currentTarget.style.color = 'var(--text2)' }}
          onMouseLeave={(e) => { e.currentTarget.style.color = 'var(--text3)' }}
        >
          ← Dashboard
        </Link>
      </div>

      {/* Title + edit button */}
      <div style={{ display: 'flex', alignItems: 'flex-start', gap: '12px', marginBottom: '16px' }}>
        <h1
          style={{
            flex: 1,
            fontFamily: 'var(--font-serif)',
            fontSize: '28px',
            fontWeight: 600,
            color: 'var(--text)',
            lineHeight: 1.25,
            margin: 0,
          }}
        >
          {note.title}
        </h1>
        <button
          onClick={editMode ? handleEditCancel : handleEditOpen}
          style={{
            padding: '5px 12px',
            background: 'var(--surface)',
            border: '1px solid var(--border)',
            borderRadius: 'var(--radius-sm)',
            color: 'var(--text3)',
            fontFamily: 'var(--font-mono)',
            fontSize: '11px',
            cursor: 'pointer',
            transition: 'border-color 150ms, color 150ms',
            flexShrink: 0,
            marginTop: '6px',
          }}
          onMouseEnter={e => { e.currentTarget.style.borderColor = 'var(--accent)'; e.currentTarget.style.color = 'var(--accent)' }}
          onMouseLeave={e => { e.currentTarget.style.borderColor = 'var(--border)'; e.currentTarget.style.color = 'var(--text3)' }}
        >
          {editMode ? 'cancel' : 'edit'}
        </button>
      </div>

      {/* Metadata row */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '12px',
          flexWrap: 'wrap',
          marginBottom: '24px',
          paddingBottom: '20px',
          borderBottom: '1px solid var(--border)',
        }}
      >
        <button
          onClick={handleCopyId}
          title="Click to copy full ID"
          aria-label="Copy note ID"
          style={{
            background: 'none',
            border: 'none',
            padding: 0,
            cursor: 'pointer',
            fontFamily: 'var(--font-mono)',
            fontSize: '11px',
            color: idCopied ? 'var(--accent)' : 'var(--text3)',
            transition: 'color 150ms',
          }}
        >
          id: {idCopied ? 'copied!' : truncateId(note.id)}
        </button>

        <span style={{ color: 'var(--border)' }}>·</span>

        <span style={{ fontFamily: 'var(--font-mono)', fontSize: '11px', color: 'var(--text3)' }}>
          created {formatDate(note.created_at)}
        </span>

        <span style={{ color: 'var(--border)' }}>·</span>

        <span style={{ fontFamily: 'var(--font-mono)', fontSize: '11px', color: 'var(--text3)' }}>
          updated {formatDate(note.updated_at)}
        </span>

        {note.tags && note.tags.length > 0 && (
          <>
            <span style={{ color: 'var(--border)' }}>·</span>
            <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap' }}>
              {note.tags.map(tag => (
                <TagChip key={tag} tag={tag} />
              ))}
            </div>
          </>
        )}
      </div>

      {/* Content / Editor */}
      {editMode ? (
        <Suspense fallback={
          <div style={{ height: 300, background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 6, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--text3)', fontFamily: 'var(--font-mono)', fontSize: '12px' }}>
            Loading editor…
          </div>
        }>
          <div style={{ marginBottom: '48px' }}>
            <SmritiEditor
              value={editContent}
              onChange={setEditContent}
              onLinkClick={handleLinkClick}
              noteId={id}
              placeholder="Write your note…"
              minHeight={400}
            />
            <div style={{ marginTop: '8px', display: 'flex', gap: '8px' }}>
              <button
                onClick={async () => {
                  try {
                    await client.updateNote(id!, { content: editContent })
                    await queryClient.invalidateQueries({ queryKey: ['note', id] })
                    setEditMode(false)
                    showToast('Note saved', 'success')
                  } catch (err) {
                    showToast(err instanceof Error ? err.message : 'Save failed', 'error')
                  }
                }}
                style={{
                  padding: '7px 16px',
                  background: 'var(--accent)',
                  color: 'var(--bg)',
                  border: 'none',
                  borderRadius: 'var(--radius-sm)',
                  fontFamily: 'var(--font-ui)',
                  fontSize: '13px',
                  fontWeight: 600,
                  cursor: 'pointer',
                }}
              >
                Save
              </button>
              <button
                onClick={handleEditCancel}
                style={{
                  padding: '7px 16px',
                  background: 'transparent',
                  color: 'var(--text3)',
                  border: '1px solid var(--border)',
                  borderRadius: 'var(--radius-sm)',
                  fontFamily: 'var(--font-ui)',
                  fontSize: '13px',
                  cursor: 'pointer',
                }}
              >
                Cancel
              </button>
            </div>
          </div>
        </Suspense>
      ) : (
        <div
          className="prose"
          dangerouslySetInnerHTML={{ __html: htmlContent }}
          style={{ marginBottom: '48px' }}
        />
      )}

      {/* Linked notes */}
      <div>
        <div
          style={{
            fontSize: '11px',
            fontFamily: 'var(--font-mono)',
            color: 'var(--text3)',
            marginBottom: '12px',
            letterSpacing: '0.08em',
            textTransform: 'uppercase',
          }}
        >
          Linked notes
        </div>

        {linksLoading ? (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
            {Array.from({ length: 3 }).map((_, i) => (
              <Skeleton key={i} height={36} />
            ))}
          </div>
        ) : !links || links.length === 0 ? (
          <div style={{ color: 'var(--text3)', fontSize: '13px', padding: '12px 0' }}>
            No linked notes
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
            {links.map(link => (
              <button
                key={link.id}
                onClick={() => navigate(`/notes/${link.id}`)}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px',
                  padding: '10px 14px',
                  background: 'var(--surface)',
                  border: '1px solid var(--border)',
                  borderRadius: 'var(--radius-sm)',
                  color: 'var(--text)',
                  fontFamily: 'var(--font-ui)',
                  fontSize: '13px',
                  textAlign: 'left',
                  cursor: 'pointer',
                  transition: 'border-color 150ms, background 150ms',
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.borderColor = 'var(--accent)'
                  e.currentTarget.style.background = 'var(--surface2)'
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.borderColor = 'var(--border)'
                  e.currentTarget.style.background = 'var(--surface)'
                }}
              >
                <span style={{ color: 'var(--accent)', fontSize: '12px' }}>→</span>
                <span style={{ flex: 1 }}>{link.title}</span>
                {Array.isArray(link.tags) && link.tags.slice(0, 2).map(tag => (
                  <TagChip key={tag} tag={tag} />
                ))}
              </button>
            ))}
          </div>
        )}
      </div>

      <Toast toasts={toasts} onDismiss={dismissToast} />
    </div>
  )
}
