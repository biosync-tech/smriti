/**
 * SmritiEditor — CodeMirror 6 editor with:
 *   - Wiki-link [[ autocomplete from /api/v1/search
 *   - Custom syntax highlighting (wiki-links, #tags, rel: types, markdown)
 *   - Click-to-navigate on wiki-links (Cmd/Ctrl+Click)
 *   - Minimal toolbar (bold, italic, code, heading, wiki-link, tag)
 *   - Word count + auto-save status bar
 *   - Auto-save with 1500ms debounce on noteId
 *
 * Lazy-loaded via React.lazy — never imported synchronously.
 */

import { useEffect, useRef, useCallback, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { EditorState, RangeSetBuilder } from '@codemirror/state'
import {
  EditorView,
  keymap,
  Decoration,
  type DecorationSet,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
  placeholder as cmPlaceholder,
} from '@codemirror/view'
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands'
import {
  autocompletion,
  completionKeymap,
  type CompletionContext,
  type Completion,
} from '@codemirror/autocomplete'
import { markdown } from '@codemirror/lang-markdown'
import { syntaxHighlighting, HighlightStyle } from '@codemirror/language'
import { tags as t } from '@lezer/highlight'
import client from '../api/client'

// ── App color palette ────────────────────────────────────────────────────────

const C = {
  bg:      '#0d0f0e',
  surface: '#1a1d1a',
  surf2:   '#242724',
  border:  '#2a2d2a',
  text:    '#e8ebe6',
  text2:   '#9ca89a',
  text3:   '#5a6359',
  accent:  '#6ee7a0',
  amber:   '#f5c842',
  error:   '#e07a7a',
}

const REL_COLORS: Record<string, string> = {
  causal:   '#f5c842',
  semantic: '#4a9eff',
  temporal: '#a78bfa',
  inspires: '#6ee7a0',
  uses:     '#3ecfcf',
  blocks:   '#e07a7a',
}

// ── Custom highlight style ────────────────────────────────────────────────────

const smritiHighlight = HighlightStyle.define([
  { tag: t.heading,          color: C.text,   fontWeight: '600' },
  { tag: t.strong,           color: C.text,   fontWeight: '600' },
  { tag: t.emphasis,         color: C.text2,  fontStyle: 'italic' },
  { tag: t.monospace,        color: C.accent, fontFamily: 'IBM Plex Mono, monospace' },
  { tag: t.link,             color: C.accent, textDecoration: 'underline' },
  { tag: t.meta,             color: C.text3 },
  { tag: t.comment,         color: C.text3 },
  { tag: t.url,              color: C.accent },
  { tag: t.processingInstruction, color: C.amber },
])

// ── Wiki-link decoration ──────────────────────────────────────────────────────

// Matches: [[rel:type|Title]] or [[Title]]
const WIKILINK_RE = /\[\[(?:rel:([a-z]+)\|)?([^\]]+)\]\]/g

class WikiLinkWidget extends WidgetType {
  constructor(readonly title: string, readonly rel: string | undefined) { super() }
  toDOM(): HTMLElement {
    const span = document.createElement('span')
    span.className = 'cm-wikilink'
    const color = this.rel ? (REL_COLORS[this.rel] ?? C.accent) : C.accent
    span.style.cssText = `color:${color};cursor:pointer;border-bottom:1px solid ${color}40;`
    span.textContent = this.rel ? `[[${this.rel}:${this.title}]]` : `[[${this.title}]]`
    span.title = 'Cmd/Ctrl+Click to open'
    return span
  }
  eq(other: WikiLinkWidget): boolean {
    return this.title === other.title && this.rel === other.rel
  }
}

function wikilinkDecorations(view: EditorView): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>()
  for (const { from, to } of view.visibleRanges) {
    const text = view.state.doc.sliceString(from, to)
    let m: RegExpExecArray | null
    WIKILINK_RE.lastIndex = 0
    while ((m = WIKILINK_RE.exec(text)) !== null) {
      const start = from + m.index
      const end = start + m[0].length
      builder.add(
        start, end,
        Decoration.replace({ widget: new WikiLinkWidget(m[2], m[1]) }),
      )
    }
  }
  return builder.finish()
}

const wikilinkPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet
    constructor(view: EditorView) { this.decorations = wikilinkDecorations(view) }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged)
        this.decorations = wikilinkDecorations(update.view)
    }
  },
  { decorations: v => v.decorations },
)

// ── #tag decoration ───────────────────────────────────────────────────────────

const TAG_RE = /#([a-zA-Z][a-zA-Z0-9_-]*)/g

function tagDecorations(view: EditorView): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>()
  for (const { from, to } of view.visibleRanges) {
    const text = view.state.doc.sliceString(from, to)
    let m: RegExpExecArray | null
    TAG_RE.lastIndex = 0
    while ((m = TAG_RE.exec(text)) !== null) {
      const start = from + m.index
      const end = start + m[0].length
      builder.add(
        start, end,
        Decoration.mark({ class: 'cm-tag', attributes: { style: `color:${C.amber}` } }),
      )
    }
  }
  return builder.finish()
}

const tagPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet
    constructor(view: EditorView) { this.decorations = tagDecorations(view) }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged)
        this.decorations = tagDecorations(update.view)
    }
  },
  { decorations: v => v.decorations },
)

// ── Wiki-link autocomplete ────────────────────────────────────────────────────

const REL_TYPES = ['causal', 'semantic', 'temporal', 'inspires', 'uses', 'blocks']

// Simple debounce for async autocomplete
let acDebounceTimer: ReturnType<typeof setTimeout>
function debounceAc<T>(fn: () => Promise<T>, ms: number): Promise<T> {
  clearTimeout(acDebounceTimer)
  return new Promise<T>((resolve, reject) => {
    acDebounceTimer = setTimeout(() => fn().then(resolve).catch(reject), ms)
  })
}

function wikiLinkCompletionSource(context: CompletionContext) {
  // Must be after [[
  const before = context.matchBefore(/\[\[([^\]]*)?/)
  if (!before || (before.from === before.to && !context.explicit)) return null

  const typed = before.text.slice(2) // strip [[
  const fromPos = before.from + 2    // character position of typed text start

  // Suggest rel: types first
  if (typed.startsWith('rel:') || typed === 'r') {
    return {
      from: before.from,
      options: REL_TYPES.map(r => ({
        label: `[[rel:${r}|`,
        displayLabel: `rel:${r}`,
        detail: 'relation type',
        apply: `[[rel:${r}|`,
      })),
    }
  }

  return debounceAc(async () => {
    const q = typed.replace(/^rel:[a-z]+\|/, '')
    if (q.length < 1) return { from: fromPos, options: [] }
    const results = await client.search(q, 'fts')
    const options: Completion[] = results.slice(0, 10).map(r => ({
      label: r.title,
      displayLabel: r.title,
      detail: r.preview?.slice(0, 60) ?? '',
      apply: (view: EditorView, _completion: Completion, _applyFrom: number, applyTo: number) => {
        // Replace from [[ to end of typed text with full wikilink
        const insertion = `[[rel:semantic|${r.title}]]`
        view.dispatch({ changes: { from: before.from, to: applyTo, insert: insertion } })
      },
    }))
    return { from: fromPos, options }
  }, 200)
}

// ── Editor theme ──────────────────────────────────────────────────────────────

const smritiTheme = EditorView.theme({
  '&': {
    background: C.surface,
    color: C.text,
    height: '100%',
    fontFamily: 'IBM Plex Mono, monospace',
    fontSize: '14px',
    lineHeight: '1.7',
  },
  '.cm-content': { caretColor: C.accent, padding: '12px 16px' },
  '.cm-focused': { outline: 'none' },
  '.cm-cursor': { borderLeftColor: C.accent, borderLeftWidth: '2px' },
  '.cm-selectionBackground': { background: `${C.accent}22` },
  '&.cm-focused .cm-selectionBackground': { background: `${C.accent}30` },
  '.cm-line': { padding: '0 2px' },
  '.cm-activeLine': { background: `${C.surf2}88` },
  '.cm-gutters': { background: C.surface, borderRight: `1px solid ${C.border}`, color: C.text3 },
  '.cm-activeLineGutter': { background: C.surf2 },
  // Autocomplete dropdown
  '.cm-tooltip': {
    background: C.surf2,
    border: `1px solid ${C.border}`,
    borderRadius: '4px',
    boxShadow: `0 4px 16px rgba(0,0,0,0.4)`,
  },
  '.cm-tooltip-autocomplete > ul > li': {
    padding: '4px 10px',
    fontFamily: 'IBM Plex Sans, sans-serif',
    fontSize: '12px',
    color: C.text2,
  },
  '.cm-tooltip-autocomplete > ul > li[aria-selected]': {
    background: `${C.accent}18`,
    color: C.accent,
  },
  '.cm-completionLabel': { color: C.text },
  '.cm-completionDetail': { color: C.text3, fontStyle: 'italic', fontSize: '11px' },
  '.cm-placeholder': { color: C.text3 },
}, { dark: true })

// ── Word count ────────────────────────────────────────────────────────────────

function wordCount(text: string): number {
  return text.trim().split(/\s+/).filter(Boolean).length
}

// ── Toolbar button ────────────────────────────────────────────────────────────

function ToolbarBtn({
  label,
  title,
  onAction,
}: {
  label: string
  title: string
  onAction: () => void
}) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      onMouseDown={(e) => {
        e.preventDefault() // keep editor focus
        onAction()
      }}
      style={{
        padding: '3px 8px',
        background: 'transparent',
        border: 'none',
        borderRadius: '3px',
        color: C.text3,
        fontFamily: 'IBM Plex Mono, monospace',
        fontSize: '13px',
        fontWeight: 600,
        cursor: 'pointer',
        transition: 'color 150ms, background 150ms',
        lineHeight: 1,
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.color = C.text
        e.currentTarget.style.background = `${C.surf2}cc`
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.color = C.text3
        e.currentTarget.style.background = 'transparent'
      }}
    >
      {label}
    </button>
  )
}

// ── Component props ───────────────────────────────────────────────────────────

export interface SmritiEditorProps {
  value: string
  onChange: (value: string) => void
  onLinkClick: (noteTitle: string) => void
  noteId?: string
  placeholder?: string
  minHeight?: number
}

// ── Main component ────────────────────────────────────────────────────────────

export default function SmritiEditor({
  value,
  onChange,
  onLinkClick,
  noteId,
  placeholder = 'Write your note…',
  minHeight = 300,
}: SmritiEditorProps) {
  const navigate = useNavigate()
  const editorRef = useRef<HTMLDivElement>(null)
  const viewRef = useRef<EditorView | null>(null)
  const onChangeRef = useRef(onChange)
  const onLinkClickRef = useRef(onLinkClick)
  const navigateRef = useRef(navigate)
  onChangeRef.current = onChange
  onLinkClickRef.current = onLinkClick
  navigateRef.current = navigate

  // Auto-save state
  const [saveStatus, setSaveStatus] = useState<'saved' | 'unsaved' | 'saving' | 'error'>('saved')
  const [saveError, setSaveError] = useState<string | null>(null)
  const [lastSaved, setLastSaved] = useState<Date | null>(null)
  const [wc, setWc] = useState(() => wordCount(value))
  const autoSaveTimer = useRef<ReturnType<typeof setTimeout>>()

  const triggerAutoSave = useCallback((content: string) => {
    if (!noteId) return
    clearTimeout(autoSaveTimer.current)
    setSaveStatus('unsaved')
    autoSaveTimer.current = setTimeout(async () => {
      setSaveStatus('saving')
      try {
        await client.updateNote(noteId, { content })
        setSaveStatus('saved')
        setSaveError(null)
        setLastSaved(new Date())
      } catch (err) {
        setSaveStatus('error')
        setSaveError(err instanceof Error ? err.message : 'Unknown error')
      }
    }, 1500)
  }, [noteId])

  // Cmd/Ctrl+Click on wikilinks
  const handleEditorClick = useCallback((e: MouseEvent) => {
    if (!(e.metaKey || e.ctrlKey)) return
    const target = e.target as HTMLElement
    if (target.classList.contains('cm-wikilink')) {
      e.preventDefault()
      e.stopPropagation()
      // Extract title from widget text: [[rel:...|Title]] or [[Title]]
      const text = target.textContent ?? ''
      const m = text.match(/\[\[(?:rel:[a-z]+\|)?([^\]]+)\]\]/)
      if (m) onLinkClickRef.current(m[1])
    }
  }, [])

  // Init editor
  useEffect(() => {
    if (!editorRef.current) return
    const el = editorRef.current

    const state = EditorState.create({
      doc: value,
      extensions: [
        history(),
        keymap.of([...defaultKeymap, ...historyKeymap, ...completionKeymap]),
        markdown(),
        syntaxHighlighting(smritiHighlight),
        wikilinkPlugin,
        tagPlugin,
        autocompletion({ override: [wikiLinkCompletionSource] }),
        smritiTheme,
        cmPlaceholder(placeholder),
        EditorView.updateListener.of((update) => {
          if (!update.docChanged) return
          const content = update.state.doc.toString()
          setWc(wordCount(content))
          onChangeRef.current(content)
          if (noteId) triggerAutoSave(content)
          else setSaveStatus('unsaved')
        }),
        EditorView.lineWrapping,
      ],
    })

    const view = new EditorView({ state, parent: el })
    viewRef.current = view

    el.addEventListener('click', handleEditorClick)

    return () => {
      clearTimeout(autoSaveTimer.current)
      el.removeEventListener('click', handleEditorClick)
      view.destroy()
      viewRef.current = null
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []) // intentionally empty — editor is uncontrolled after init

  // Sync external value changes (e.g. when note data loads)
  useEffect(() => {
    const view = viewRef.current
    if (!view) return
    const current = view.state.doc.toString()
    if (current !== value) {
      view.dispatch({
        changes: { from: 0, to: current.length, insert: value },
      })
      setWc(wordCount(value))
    }
  }, [value])

  // Toolbar actions
  function insertAt(before: string, after: string = '') {
    const view = viewRef.current
    if (!view) return
    const { from, to } = view.state.selection.main
    const selected = view.state.doc.sliceString(from, to)
    view.dispatch({
      changes: { from, to, insert: before + selected + after },
      selection: { anchor: from + before.length, head: from + before.length + selected.length },
    })
    view.focus()
  }

  function insertAtLineStart(prefix: string) {
    const view = viewRef.current
    if (!view) return
    const line = view.state.doc.lineAt(view.state.selection.main.from)
    view.dispatch({
      changes: { from: line.from, insert: prefix },
      selection: { anchor: line.from + prefix.length },
    })
    view.focus()
  }

  function insertWikiLink() {
    const view = viewRef.current
    if (!view) return
    const { from } = view.state.selection.main
    view.dispatch({ changes: { from, insert: '[[' } })
    view.focus()
  }

  // Save status label
  const statusLabel = (() => {
    if (!noteId) return wc + ' words'
    switch (saveStatus) {
      case 'saving': return '…saving'
      case 'saved':  return lastSaved
        ? `saved ${lastSaved.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' })}`
        : 'saved'
      case 'error':  return saveError ? `error — ${saveError}` : 'error'
      case 'unsaved': return 'unsaved changes'
    }
  })()

  const statusColor = saveStatus === 'saved'
    ? C.accent
    : saveStatus === 'error'
      ? C.error
      : saveStatus === 'saving'
        ? C.text3
        : C.amber

  void navigateRef // used via ref

  return (
    <div style={{ display: 'flex', flexDirection: 'column', border: `1px solid ${C.border}`, borderRadius: '6px', overflow: 'hidden' }}>
      {/* Toolbar */}
      <div
        role="toolbar"
        aria-label="Text formatting"
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '2px',
          padding: '4px 8px',
          background: C.surface,
          borderBottom: `1px solid ${C.border}`,
          flexWrap: 'wrap',
        }}
      >
        <ToolbarBtn label="B"  title="Bold (wrap selection)"    onAction={() => insertAt('**', '**')} />
        <ToolbarBtn label="I"  title="Italic (wrap selection)"  onAction={() => insertAt('*', '*')} />
        <ToolbarBtn label="`"  title="Inline code"              onAction={() => insertAt('`', '`')} />
        <ToolbarBtn label="#"  title="Heading"                  onAction={() => insertAtLineStart('## ')} />
        <div style={{ width: 1, height: 16, background: C.border, margin: '0 4px' }} />
        <ToolbarBtn label="[["  title="Insert wiki-link"        onAction={insertWikiLink} />
        <ToolbarBtn label="#tag" title="Insert tag"             onAction={() => insertAt('#', '')} />
      </div>

      {/* Editor */}
      <div
        ref={editorRef}
        style={{ minHeight, flex: 1, background: C.surface }}
      />

      {/* Status bar */}
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          padding: '4px 12px',
          background: C.surface,
          borderTop: `1px solid ${C.border}`,
          fontSize: '11px',
          fontFamily: 'IBM Plex Mono, monospace',
          color: C.text3,
        }}
      >
        <span>{wc} words</span>
        {noteId && (
          <span
            style={{ color: statusColor, cursor: saveStatus === 'error' ? 'pointer' : 'default' }}
            onClick={() => {
              if (saveStatus === 'error' && viewRef.current) {
                triggerAutoSave(viewRef.current.state.doc.toString())
              }
            }}
          >
            {statusLabel}
          </span>
        )}
      </div>
    </div>
  )
}
