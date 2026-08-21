import type { ToastItem } from '../hooks/useToast'

interface ToastProps {
  toasts: ToastItem[]
  onDismiss: (id: string) => void
}

export default function Toast({ toasts, onDismiss }: ToastProps) {
  if (toasts.length === 0) return null

  return (
    <div
      style={{
        position: 'fixed',
        bottom: '24px',
        right: '24px',
        display: 'flex',
        flexDirection: 'column',
        gap: '8px',
        zIndex: 1000,
        maxWidth: '360px',
      }}
      aria-live="polite"
      aria-atomic="false"
    >
      {toasts.map(toast => (
        <div
          key={toast.id}
          role="alert"
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            gap: '12px',
            padding: '12px 16px',
            background: 'var(--surface)',
            border: `1px solid ${toast.type === 'error' ? 'var(--error)' : 'var(--accent)'}`,
            borderRadius: 'var(--radius-md)',
            boxShadow: '0 4px 16px rgba(0,0,0,0.4)',
            animation: 'slideIn 150ms ease-out',
          }}
        >
          <span
            style={{
              fontSize: '13px',
              color: toast.type === 'error' ? 'var(--error)' : 'var(--text)',
              fontFamily: 'var(--font-ui)',
            }}
          >
            {toast.type === 'success' ? (
              <span style={{ color: 'var(--accent)', marginRight: '8px' }}>✓</span>
            ) : (
              <span style={{ color: 'var(--error)', marginRight: '8px' }}>✕</span>
            )}
            {toast.message}
          </span>
          <button
            onClick={() => onDismiss(toast.id)}
            aria-label="Dismiss notification"
            style={{
              background: 'none',
              border: 'none',
              padding: '2px 6px',
              color: 'var(--text3)',
              cursor: 'pointer',
              fontSize: '16px',
              lineHeight: 1,
              borderRadius: 'var(--radius-sm)',
              flexShrink: 0,
            }}
          >
            ×
          </button>
        </div>
      ))}
      <style>{`
        @keyframes slideIn {
          from { opacity: 0; transform: translateY(8px); }
          to   { opacity: 1; transform: translateY(0); }
        }
      `}</style>
    </div>
  )
}
