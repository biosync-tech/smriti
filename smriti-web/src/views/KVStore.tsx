import { useState, useRef } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import client from '../api/client'
import type { KVEntry } from '../api/types'
import Skeleton from '../components/Skeleton'
import Toast from '../components/Toast'
import { useToast } from '../hooks/useToast'

function formatTTL(ttl: number | null): string {
  if (ttl === null || ttl === undefined) return '∞'
  return `${ttl}s`
}

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleDateString('en-US', {
      month: 'short', day: 'numeric', year: 'numeric',
    })
  } catch {
    return iso
  }
}

interface EditingCell {
  id: string
  value: string
}

export default function KVStore() {
  const queryClient = useQueryClient()
  const { toasts, showToast, dismissToast } = useToast()

  const [prefix, setPrefix] = useState('')
  const [showAddForm, setShowAddForm] = useState(false)
  const [editingCell, setEditingCell] = useState<EditingCell | null>(null)
  const editInputRef = useRef<HTMLInputElement>(null)

  // New entry form state
  const [newKey, setNewKey] = useState('')
  const [newValue, setNewValue] = useState('')
  const [newNamespace, setNewNamespace] = useState('default')
  const [newTTL, setNewTTL] = useState('')
  const [adding, setAdding] = useState(false)

  // Optimistic delete tracking
  const [deletingIds, setDeletingIds] = useState<Set<string>>(new Set())

  const { data: entries, isLoading } = useQuery({
    queryKey: ['kv', prefix],
    queryFn: () => client.listKV(prefix || undefined),
  })

  async function handleAdd(e: React.FormEvent) {
    e.preventDefault()
    if (!newKey.trim()) return
    setAdding(true)
    try {
      let parsedValue: unknown = newValue
      try { parsedValue = JSON.parse(newValue) } catch { /* keep as string */ }

      const ttl = newTTL ? parseInt(newTTL, 10) : undefined
      await client.setKV(newKey.trim(), parsedValue, ttl)
      showToast('Entry added', 'success')
      setNewKey('')
      setNewValue('')
      setNewNamespace('default')
      setNewTTL('')
      setShowAddForm(false)
      void queryClient.invalidateQueries({ queryKey: ['kv'] })
    } catch (err) {
      showToast(err instanceof Error ? err.message : 'Failed to add entry', 'error')
    } finally {
      setAdding(false)
    }
  }

  async function handleDelete(entry: KVEntry) {
    setDeletingIds(prev => new Set([...prev, entry.id]))
    try {
      await client.deleteKV(entry.key)
      void queryClient.invalidateQueries({ queryKey: ['kv'] })
      showToast('Entry deleted', 'success')
    } catch (err) {
      showToast(err instanceof Error ? err.message : 'Failed to delete', 'error')
      setDeletingIds(prev => { const s = new Set(prev); s.delete(entry.id); return s })
    }
  }

  function startEditing(entry: KVEntry) {
    const displayValue = typeof entry.value === 'string'
      ? entry.value
      : JSON.stringify(entry.value)
    setEditingCell({ id: entry.id, value: displayValue })
    setTimeout(() => editInputRef.current?.focus(), 0)
  }

  async function commitEdit(entry: KVEntry) {
    if (!editingCell) return
    const raw = editingCell.value
    let parsed: unknown = raw
    try { parsed = JSON.parse(raw) } catch { /* keep as string */ }

    const orig = typeof entry.value === 'string'
      ? entry.value
      : JSON.stringify(entry.value)
    if (raw === orig) {
      setEditingCell(null)
      return
    }

    try {
      await client.setKV(entry.key, parsed, entry.ttl_seconds ?? undefined)
      void queryClient.invalidateQueries({ queryKey: ['kv'] })
      showToast('Entry updated', 'success')
    } catch (err) {
      showToast(err instanceof Error ? err.message : 'Failed to update', 'error')
    } finally {
      setEditingCell(null)
    }
  }

  const displayedEntries = entries?.filter(e => !deletingIds.has(e.id)) ?? []

  const thStyle: React.CSSProperties = {
    padding: '8px 12px',
    textAlign: 'left',
    fontFamily: 'var(--font-mono)',
    fontSize: '10px',
    letterSpacing: '0.08em',
    textTransform: 'uppercase' as const,
    color: 'var(--text3)',
    borderBottom: '1px solid var(--border)',
    fontWeight: 500,
    whiteSpace: 'nowrap',
  }

  return (
    <div style={{ padding: '32px 32px 48px', maxWidth: '1100px' }}>

      {/* Header */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          marginBottom: '24px',
          gap: '16px',
          flexWrap: 'wrap',
        }}
      >
        <div>
          <h1
            style={{
              fontFamily: 'var(--font-serif)',
              fontSize: '22px',
              fontWeight: 600,
              color: 'var(--text)',
              marginBottom: '2px',
            }}
          >
            KV Store
          </h1>
          <div style={{ fontSize: '12px', color: 'var(--text3)', fontFamily: 'var(--font-mono)' }}>
            {displayedEntries.length} {displayedEntries.length === 1 ? 'entry' : 'entries'}
          </div>
        </div>

        <button
          className="primary"
          onClick={() => setShowAddForm(v => !v)}
          aria-expanded={showAddForm}
        >
          {showAddForm ? '× Cancel' : '＋ Add entry'}
        </button>
      </div>

      {/* Add form */}
      {showAddForm && (
        <form
          onSubmit={handleAdd}
          style={{
            background: 'var(--surface)',
            border: '1px solid var(--border)',
            borderRadius: 'var(--radius-md)',
            padding: '20px',
            marginBottom: '24px',
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fill, minmax(180px, 1fr))',
            gap: '12px',
          }}
        >
          <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
            <label style={{ fontSize: '11px', color: 'var(--text3)', fontFamily: 'var(--font-mono)' }}>
              Key *
            </label>
            <input
              autoFocus
              value={newKey}
              onChange={(e) => setNewKey(e.target.value)}
              placeholder="my_key"
              required
              aria-label="Key"
              style={{ width: '100%' }}
            />
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
            <label style={{ fontSize: '11px', color: 'var(--text3)', fontFamily: 'var(--font-mono)' }}>
              Value
            </label>
            <input
              value={newValue}
              onChange={(e) => setNewValue(e.target.value)}
              placeholder='"string" or JSON'
              aria-label="Value"
              style={{ width: '100%' }}
            />
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
            <label style={{ fontSize: '11px', color: 'var(--text3)', fontFamily: 'var(--font-mono)' }}>
              Namespace
            </label>
            <input
              value={newNamespace}
              onChange={(e) => setNewNamespace(e.target.value)}
              placeholder="default"
              aria-label="Namespace"
              style={{ width: '100%' }}
            />
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
            <label style={{ fontSize: '11px', color: 'var(--text3)', fontFamily: 'var(--font-mono)' }}>
              TTL (seconds)
            </label>
            <input
              value={newTTL}
              onChange={(e) => setNewTTL(e.target.value)}
              placeholder="optional"
              type="number"
              min="1"
              aria-label="TTL in seconds"
              style={{ width: '100%' }}
            />
          </div>

          <div style={{ display: 'flex', alignItems: 'flex-end', gap: '8px', gridColumn: '1 / -1', justifyContent: 'flex-end' }}>
            <button type="button" onClick={() => setShowAddForm(false)}>Cancel</button>
            <button type="submit" className="primary" disabled={adding || !newKey.trim()}>
              {adding ? 'Adding…' : 'Add'}
            </button>
          </div>
        </form>
      )}

      {/* Prefix filter */}
      <div style={{ marginBottom: '20px' }}>
        <input
          value={prefix}
          onChange={(e) => setPrefix(e.target.value)}
          placeholder="Filter by prefix…"
          aria-label="Filter KV entries by prefix"
          style={{ width: '280px' }}
        />
      </div>

      {/* Table */}
      {isLoading ? (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
          {Array.from({ length: 5 }).map((_, i) => (
            <Skeleton key={i} height={44} />
          ))}
        </div>
      ) : displayedEntries.length === 0 ? (
        <div
          style={{
            textAlign: 'center',
            padding: '60px 24px',
            background: 'var(--surface)',
            border: '1px solid var(--border)',
            borderRadius: 'var(--radius-md)',
            color: 'var(--text3)',
          }}
        >
          <div style={{ fontSize: '28px', marginBottom: '12px', opacity: 0.4 }}>◫</div>
          <div>No KV entries. Add one above.</div>
        </div>
      ) : (
        <div style={{ overflowX: 'auto' }}>
          <table
            style={{
              width: '100%',
              borderCollapse: 'collapse',
              background: 'var(--surface)',
              border: '1px solid var(--border)',
              borderRadius: 'var(--radius-md)',
              overflow: 'hidden',
            }}
          >
            <thead>
              <tr style={{ background: 'var(--surface)' }}>
                <th style={thStyle}>Key</th>
                <th style={thStyle}>Value</th>
                <th style={thStyle}>Namespace</th>
                <th style={thStyle}>TTL</th>
                <th style={thStyle}>Updated</th>
                <th style={{ ...thStyle, textAlign: 'right' }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {displayedEntries.map((entry, idx) => {
                const isEditing = editingCell?.id === entry.id
                const displayValue = typeof entry.value === 'string'
                  ? entry.value
                  : JSON.stringify(entry.value)

                return (
                  <tr
                    key={entry.id}
                    style={{
                      borderTop: idx > 0 ? '1px solid var(--border)' : 'none',
                      transition: 'background 150ms',
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.background = 'var(--surface2)'
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.background = 'transparent'
                    }}
                  >
                    {/* Key */}
                    <td
                      style={{
                        padding: '10px 12px',
                        fontFamily: 'var(--font-mono)',
                        fontSize: '12px',
                        color: 'var(--accent)',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {entry.key}
                    </td>

                    {/* Value (editable) */}
                    <td
                      style={{
                        padding: '10px 12px',
                        fontFamily: 'var(--font-mono)',
                        fontSize: '12px',
                        color: 'var(--text)',
                        maxWidth: '300px',
                        cursor: 'text',
                      }}
                      title="Click to edit"
                    >
                      {isEditing ? (
                        <input
                          ref={editInputRef}
                          value={editingCell.value}
                          onChange={(e) => setEditingCell({ ...editingCell, value: e.target.value })}
                          onBlur={() => void commitEdit(entry)}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter') void commitEdit(entry)
                            if (e.key === 'Escape') setEditingCell(null)
                          }}
                          style={{ width: '100%', fontFamily: 'var(--font-mono)', fontSize: '12px' }}
                          aria-label={`Edit value for ${entry.key}`}
                        />
                      ) : (
                        <span
                          role="button"
                          tabIndex={0}
                          onClick={() => startEditing(entry)}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter' || e.key === ' ') {
                              e.preventDefault()
                              startEditing(entry)
                            }
                          }}
                          style={{
                            display: 'block',
                            overflow: 'hidden',
                            textOverflow: 'ellipsis',
                            whiteSpace: 'nowrap',
                            cursor: 'text',
                          }}
                        >
                          {displayValue}
                        </span>
                      )}
                    </td>

                    {/* Namespace */}
                    <td
                      style={{
                        padding: '10px 12px',
                        fontFamily: 'var(--font-mono)',
                        fontSize: '12px',
                        color: 'var(--text2)',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {entry.namespace}
                    </td>

                    {/* TTL */}
                    <td
                      style={{
                        padding: '10px 12px',
                        fontFamily: 'var(--font-mono)',
                        fontSize: '12px',
                        color: entry.ttl_seconds ? 'var(--amber)' : 'var(--text3)',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {formatTTL(entry.ttl_seconds)}
                    </td>

                    {/* Updated */}
                    <td
                      style={{
                        padding: '10px 12px',
                        fontFamily: 'var(--font-mono)',
                        fontSize: '11px',
                        color: 'var(--text3)',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {formatDate(entry.updated_at)}
                    </td>

                    {/* Actions */}
                    <td style={{ padding: '10px 12px', textAlign: 'right' }}>
                      <button
                        onClick={() => void handleDelete(entry)}
                        aria-label={`Delete entry ${entry.key}`}
                        title="Delete entry"
                        style={{
                          background: 'none',
                          border: '1px solid transparent',
                          padding: '4px 8px',
                          color: 'var(--text3)',
                          cursor: 'pointer',
                          borderRadius: 'var(--radius-sm)',
                          fontSize: '14px',
                          transition: 'color 150ms, border-color 150ms',
                        }}
                        onMouseEnter={(e) => {
                          e.currentTarget.style.color = 'var(--error)'
                          e.currentTarget.style.borderColor = 'var(--error)'
                        }}
                        onMouseLeave={(e) => {
                          e.currentTarget.style.color = 'var(--text3)'
                          e.currentTarget.style.borderColor = 'transparent'
                        }}
                      >
                        🗑
                      </button>
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      )}

      <Toast toasts={toasts} onDismiss={dismissToast} />
    </div>
  )
}
