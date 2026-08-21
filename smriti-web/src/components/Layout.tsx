import { useState, type ReactNode } from 'react'
import { NavLink, useLocation } from 'react-router-dom'

interface NavItem {
  path: string
  label: string
  icon: string
  disabled?: boolean
  tooltip?: string
}

const NAV_ITEMS: NavItem[] = [
  { path: '/', label: 'Dashboard', icon: '▦' },
  { path: '/search', label: 'Search', icon: '⌕' },
  { path: '/kv', label: 'KV Store', icon: '◫' },
  { path: '/graph', label: 'Graph', icon: '◎' },
]

interface LayoutProps {
  children: ReactNode
}

export default function Layout({ children }: LayoutProps) {
  const [expanded, setExpanded] = useState(false)
  const location = useLocation()

  return (
    <div style={{ display: 'flex', minHeight: '100vh', background: 'var(--bg)' }}>
      {/* Desktop sidebar */}
      <nav
        role="navigation"
        aria-label="Main navigation"
        onMouseEnter={() => setExpanded(true)}
        onMouseLeave={() => setExpanded(false)}
        style={{
          display: 'flex',
          flexDirection: 'column',
          position: 'fixed',
          top: 0,
          left: 0,
          height: '100vh',
          width: expanded ? '200px' : '64px',
          background: 'var(--surface)',
          borderRight: '1px solid var(--border)',
          transition: 'width 200ms ease',
          zIndex: 100,
          overflow: 'hidden',
          paddingTop: '16px',
        }}
        className="sidebar"
      >
        {/* Logo / wordmark */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '10px',
            padding: '8px 16px',
            marginBottom: '24px',
            whiteSpace: 'nowrap',
            overflow: 'hidden',
          }}
        >
          <span style={{ fontSize: '20px', flexShrink: 0, lineHeight: 1 }}>◈</span>
          {expanded && (
            <span
              style={{
                fontFamily: 'var(--font-serif)',
                fontSize: '16px',
                fontWeight: 600,
                color: 'var(--text)',
                letterSpacing: '0.02em',
              }}
            >
              Smriti
            </span>
          )}
        </div>

        {/* Nav links */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: '2px', padding: '0 8px' }}>
          {NAV_ITEMS.map(item => {
            const isActive = item.path === '/'
              ? location.pathname === '/'
              : location.pathname.startsWith(item.path)

            if (item.disabled) {
              return (
                <div
                  key={item.path}
                  title={item.tooltip}
                  aria-label={`${item.label} — ${item.tooltip ?? 'disabled'}`}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '12px',
                    padding: '10px 8px',
                    borderRadius: 'var(--radius-md)',
                    color: 'var(--text3)',
                    opacity: 0.5,
                    cursor: 'not-allowed',
                    whiteSpace: 'nowrap',
                    overflow: 'hidden',
                    userSelect: 'none',
                    position: 'relative',
                  }}
                >
                  <span style={{ fontSize: '18px', flexShrink: 0, width: '24px', textAlign: 'center' }}>
                    {item.icon}
                  </span>
                  {expanded && (
                    <span style={{ fontSize: '13px', fontFamily: 'var(--font-ui)' }}>
                      {item.label}
                    </span>
                  )}
                </div>
              )
            }

            return (
              <NavLink
                key={item.path}
                to={item.path}
                end={item.path === '/'}
                aria-label={item.label}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '12px',
                  padding: '10px 8px',
                  borderRadius: 'var(--radius-md)',
                  color: isActive ? 'var(--accent)' : 'var(--text2)',
                  background: isActive ? 'rgba(110,231,160,0.08)' : 'transparent',
                  borderLeft: isActive ? '2px solid var(--accent)' : '2px solid transparent',
                  textDecoration: 'none',
                  transition: 'color 150ms, background 150ms',
                  whiteSpace: 'nowrap',
                  overflow: 'hidden',
                  fontFamily: 'var(--font-ui)',
                }}
                onMouseEnter={(e) => {
                  if (!isActive) {
                    e.currentTarget.style.color = 'var(--text)'
                    e.currentTarget.style.background = 'rgba(255,255,255,0.04)'
                  }
                }}
                onMouseLeave={(e) => {
                  if (!isActive) {
                    e.currentTarget.style.color = 'var(--text2)'
                    e.currentTarget.style.background = 'transparent'
                  }
                }}
              >
                <span style={{ fontSize: '18px', flexShrink: 0, width: '24px', textAlign: 'center' }}>
                  {item.icon}
                </span>
                {expanded && (
                  <span style={{ fontSize: '13px' }}>{item.label}</span>
                )}
              </NavLink>
            )
          })}
        </div>

        {/* Version tag */}
        {expanded && (
          <div
            style={{
              marginTop: 'auto',
              padding: '16px',
              fontSize: '11px',
              fontFamily: 'var(--font-mono)',
              color: 'var(--text3)',
            }}
          >
            v0.1.0
          </div>
        )}
      </nav>

      {/* Mobile bottom bar */}
      <nav
        role="navigation"
        aria-label="Mobile navigation"
        style={{
          display: 'none',
          position: 'fixed',
          bottom: 0,
          left: 0,
          right: 0,
          height: '56px',
          background: 'var(--surface)',
          borderTop: '1px solid var(--border)',
          zIndex: 100,
          padding: '0 8px',
        }}
        className="mobile-nav"
      >
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-around',
            height: '100%',
          }}
        >
          {NAV_ITEMS.filter(item => !item.disabled).map(item => {
            const isActive = item.path === '/'
              ? location.pathname === '/'
              : location.pathname.startsWith(item.path)

            return (
              <NavLink
                key={item.path}
                to={item.path}
                end={item.path === '/'}
                aria-label={item.label}
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  alignItems: 'center',
                  gap: '2px',
                  padding: '8px 16px',
                  color: isActive ? 'var(--accent)' : 'var(--text3)',
                  textDecoration: 'none',
                  borderRadius: 'var(--radius-sm)',
                  transition: 'color 150ms',
                }}
              >
                <span style={{ fontSize: '20px', lineHeight: 1 }}>{item.icon}</span>
                <span style={{ fontSize: '10px', fontFamily: 'var(--font-ui)' }}>{item.label}</span>
              </NavLink>
            )
          })}
        </div>
      </nav>

      {/* Main content */}
      <main
        style={{
          flex: 1,
          marginLeft: '64px',
          minHeight: '100vh',
          overflowX: 'hidden',
        }}
        className="main-content"
      >
        {children}
      </main>

      <style>{`
        @media (max-width: 639px) {
          .sidebar { display: none !important; }
          .mobile-nav { display: block !important; }
          .main-content { margin-left: 0 !important; padding-bottom: 64px; }
        }
      `}</style>
    </div>
  )
}
