interface TagChipProps {
  tag: string
  active?: boolean
  onClick?: () => void
}

export default function TagChip({ tag, active = false, onClick }: TagChipProps) {
  const isInteractive = onClick !== undefined

  return (
    <span
      role={isInteractive ? 'button' : undefined}
      tabIndex={isInteractive ? 0 : undefined}
      onClick={onClick}
      onKeyDown={isInteractive ? (e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          onClick()
        }
      } : undefined}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        padding: '2px 8px',
        borderRadius: '10px',
        fontSize: '11px',
        fontFamily: 'var(--font-mono)',
        background: active ? 'var(--accent)' : 'var(--surface2)',
        color: active ? 'var(--bg)' : 'var(--accent)',
        border: '1px solid var(--border)',
        cursor: isInteractive ? 'pointer' : 'default',
        userSelect: 'none',
        transition: 'background 120ms, color 120ms',
        whiteSpace: 'nowrap',
        lineHeight: '1.4',
      }}
    >
      #{tag}
    </span>
  )
}
