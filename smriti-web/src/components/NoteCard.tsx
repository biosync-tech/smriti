import type { NoteSummary } from '../api/types'
import TagChip from './TagChip'

interface NoteCardProps {
  note: NoteSummary
  onClick: () => void
}

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    })
  } catch {
    return iso
  }
}

export default function NoteCard({ note, onClick }: NoteCardProps) {
  const tags = Array.isArray(note.tags) ? note.tags : []

  return (
    <div
      role="article"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          onClick()
        }
      }}
      style={{
        background: 'var(--surface)',
        border: '1px solid var(--border)',
        borderRadius: 'var(--radius-md)',
        padding: '16px',
        cursor: 'pointer',
        transition: 'border-color 150ms, background 150ms',
        display: 'flex',
        flexDirection: 'column',
        gap: '8px',
        outline: 'none',
      }}
      onMouseEnter={(e) => {
        const el = e.currentTarget as HTMLDivElement
        el.style.borderColor = 'var(--accent)'
        el.style.background = 'var(--surface2)'
      }}
      onMouseLeave={(e) => {
        const el = e.currentTarget as HTMLDivElement
        el.style.borderColor = 'var(--border)'
        el.style.background = 'var(--surface)'
      }}
      onFocus={(e) => {
        const el = e.currentTarget as HTMLDivElement
        el.style.borderColor = 'var(--accent)'
        el.style.outline = '2px solid var(--accent)'
        el.style.outlineOffset = '2px'
      }}
      onBlur={(e) => {
        const el = e.currentTarget as HTMLDivElement
        el.style.borderColor = 'var(--border)'
        el.style.outline = 'none'
      }}
    >
      {/* Title */}
      <div
        style={{
          fontFamily: 'var(--font-serif)',
          fontSize: '16px',
          fontWeight: 600,
          color: 'var(--text)',
          lineHeight: 1.3,
          overflow: 'hidden',
          display: '-webkit-box',
          WebkitLineClamp: 2,
          WebkitBoxOrient: 'vertical',
        }}
      >
        {note.title}
      </div>

      {/* Preview */}
      {note.preview && (
        <div
          style={{
            fontFamily: 'var(--font-ui)',
            fontSize: '12px',
            color: 'var(--text2)',
            lineHeight: 1.5,
            overflow: 'hidden',
            display: '-webkit-box',
            WebkitLineClamp: 2,
            WebkitBoxOrient: 'vertical',
          }}
        >
          {note.preview.slice(0, 100)}
        </div>
      )}

      {/* Tags */}
      {tags.length > 0 && (
        <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap' }}>
          {tags.slice(0, 4).map(tag => (
            <TagChip key={tag} tag={tag} />
          ))}
          {tags.length > 4 && (
            <span style={{ fontSize: '11px', color: 'var(--text3)', alignSelf: 'center' }}>
              +{tags.length - 4}
            </span>
          )}
        </div>
      )}

      {/* Footer */}
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginTop: 'auto',
          paddingTop: '4px',
        }}
      >
        <span style={{ fontSize: '11px', color: 'var(--text3)', fontFamily: 'var(--font-mono)' }}>
          {note.backlink_count} {note.backlink_count === 1 ? 'link' : 'links'}
        </span>
        <span style={{ fontSize: '11px', color: 'var(--text3)', fontFamily: 'var(--font-mono)' }}>
          {formatDate(note.updated_at)}
        </span>
      </div>
    </div>
  )
}
