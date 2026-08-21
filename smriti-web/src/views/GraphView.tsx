import { useEffect, useRef, useState, useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import * as d3 from 'd3'
import client from '../api/client'
import type { FullGraphNode } from '../api/types'

// ── Constants ────────────────────────────────────────────────────────────────

const TAG_COLORS: Record<string, string> = {
  client:   '#4a9eff',
  project:  '#3ecfcf',
  decision: '#f5c842',
  sop:      '#a78bfa',
  research: '#f28b72',
  idea:     '#6ee7a0',
}
const NO_TAG_COLOR  = '#5a6359'
const DIM_OPACITY   = 0.12
const LINK_COLORS: Record<string, string> = {
  causal:   '#f5c842',
  semantic: '#4a9eff',
  temporal: '#a78bfa',
  inspires: '#6ee7a0',
  uses:     '#3ecfcf',
  blocks:   '#e07a7a',
  wikilink: '#5a6359',
  backlink: '#5a6359',
}
const DEFAULT_LINK_COLOR = '#3a3d3a'
const MIN_R = 6
const MAX_R = 22

function nodeRadius(lc: number): number {
  return MIN_R + Math.min(lc, 20) / 20 * (MAX_R - MIN_R)
}
function nodeColor(primary_tag: string | null): string {
  return primary_tag ? (TAG_COLORS[primary_tag] ?? NO_TAG_COLOR) : NO_TAG_COLOR
}

// Is this node "new" (< 24h old)?
function isNewNode(created_at: string): boolean {
  return Date.now() - new Date(created_at).getTime() < 86_400_000
}

// ── Types for d3 simulation ──────────────────────────────────────────────────

interface SimNode extends d3.SimulationNodeDatum, FullGraphNode {
  x: number
  y: number
  fx: number | null
  fy: number | null
  _highlight?: boolean
  _dim?: boolean
}

interface SimLink extends d3.SimulationLinkDatum<SimNode> {
  source: SimNode
  target: SimNode
  rel_type: string
  valid_from?: string
  valid_until?: string
  expired: boolean
}

// ── SVG renderer (≤ 150 nodes) ───────────────────────────────────────────────

function useSvgGraph(
  containerRef: React.RefObject<HTMLDivElement | null>,
  nodes: SimNode[],
  links: SimLink[],
  showLabels: boolean,
  onNodeClick: (n: SimNode) => void,
  onNodeDblClick: (n: SimNode) => void,
  onBackgroundDblClick: () => void,
  selectedIds: Set<string>,
) {
  const svgRef = useRef<SVGSVGElement | null>(null)
  const simRef = useRef<d3.Simulation<SimNode, SimLink> | null>(null)

  useEffect(() => {
    if (!containerRef.current || nodes.length === 0) return
    const container = containerRef.current
    const W = container.clientWidth
    const H = container.clientHeight

    // Clean up previous
    d3.select(container).select('svg').remove()
    simRef.current?.stop()

    const svg = d3.select(container)
      .append('svg')
      .attr('width', W)
      .attr('height', H)
      .style('position', 'absolute')
      .style('inset', '0')

    svgRef.current = svg.node()

    // Arrow marker
    const defs = svg.append('defs')
    Object.entries(LINK_COLORS).forEach(([type, color]) => {
      defs.append('marker')
        .attr('id', `arrow-${type}`)
        .attr('viewBox', '0 -4 8 8')
        .attr('refX', 14)
        .attr('refY', 0)
        .attr('markerWidth', 6)
        .attr('markerHeight', 6)
        .attr('orient', 'auto')
        .append('path')
        .attr('d', 'M0,-4L8,0L0,4')
        .attr('fill', color)
        .attr('opacity', 0.7)
    })
    defs.append('marker')
      .attr('id', 'arrow-default')
      .attr('viewBox', '0 -4 8 8').attr('refX', 14).attr('refY', 0)
      .attr('markerWidth', 6).attr('markerHeight', 6).attr('orient', 'auto')
      .append('path').attr('d', 'M0,-4L8,0L0,4')
      .attr('fill', DEFAULT_LINK_COLOR).attr('opacity', 0.7)

    // Zoom layer
    const g = svg.append('g')

    svg.call(
      d3.zoom<SVGSVGElement, unknown>()
        .scaleExtent([0.2, 4])
        .on('zoom', (e) => g.attr('transform', e.transform))
    )

    svg.on('dblclick.zoom', null) // separate from app dblclick

    // Background dbl-click to unpin
    svg.on('dblclick', (e) => {
      if ((e.target as Element).tagName === 'svg' ||
          (e.target as Element).tagName === 'rect') {
        onBackgroundDblClick()
      }
    })

    // Links
    const linkSel = g.append('g')
      .attr('stroke-linecap', 'round')
      .selectAll<SVGLineElement, SimLink>('line')
      .data(links)
      .join('line')
      .attr('stroke', d => LINK_COLORS[d.rel_type] ?? DEFAULT_LINK_COLOR)
      .attr('stroke-width', 1)
      .attr('stroke-dasharray', d => d.expired ? '4,3' : null)
      .attr('stroke-opacity', 0.5)
      .attr('marker-end', d => `url(#arrow-${LINK_COLORS[d.rel_type] ? d.rel_type : 'default'})`)

    // Node groups
    const nodeSel = g.append('g')
      .selectAll<SVGGElement, SimNode>('g')
      .data(nodes)
      .join('g')
      .style('cursor', 'pointer')

    // Circle
    nodeSel.append('circle')
      .attr('r', d => nodeRadius(d.link_count))
      .attr('fill', d => nodeColor(d.primary_tag))
      .attr('stroke', d => selectedIds.has(d.id) ? '#fff' : 'transparent')
      .attr('stroke-width', 2)

    // Pulse ring for new nodes
    nodeSel.filter(d => isNewNode(d.created_at))
      .append('circle')
      .attr('r', d => nodeRadius(d.link_count) + 4)
      .attr('fill', 'none')
      .attr('stroke', d => nodeColor(d.primary_tag))
      .attr('stroke-width', 1.5)
      .attr('opacity', 0.5)
      .attr('class', 'pulse-ring')

    // Labels (always for large nodes, conditionally for rest)
    nodeSel.filter(d => showLabels || d.link_count > 3)
      .append('text')
      .text(d => d.title.length > 20 ? d.title.slice(0, 18) + '…' : d.title)
      .attr('x', d => nodeRadius(d.link_count) + 4)
      .attr('y', 4)
      .attr('fill', 'var(--text2)')
      .attr('font-size', '11px')
      .attr('font-family', 'var(--font-ui)')
      .attr('pointer-events', 'none')

    // Tooltip label (hidden, shown on hover)
    const hoverLabel = nodeSel.filter(d => !showLabels && d.link_count <= 3)
      .append('text')
      .text(d => d.title.length > 24 ? d.title.slice(0, 22) + '…' : d.title)
      .attr('x', d => nodeRadius(d.link_count) + 4)
      .attr('y', 4)
      .attr('fill', 'var(--text)')
      .attr('font-size', '11px')
      .attr('font-family', 'var(--font-ui)')
      .attr('pointer-events', 'none')
      .attr('opacity', 0)

    // Hover interactions
    nodeSel
      .on('mouseenter', function(_, d) {
        // Highlight node + neighbors
        const neighborIds = new Set<string>()
        neighborIds.add(d.id)
        links.forEach(l => {
          if (l.source.id === d.id) neighborIds.add(l.target.id)
          if (l.target.id === d.id) neighborIds.add(l.source.id)
        })
        nodeSel.selectAll<SVGCircleElement, SimNode>('circle:first-child')
          .attr('opacity', n => neighborIds.has(n.id) ? 1 : DIM_OPACITY)
        linkSel.attr('stroke-opacity', l =>
          (neighborIds.has(l.source.id) && neighborIds.has(l.target.id)) ? 0.8 : 0.04
        )
        d3.select(this).select('text:last-child').attr('opacity', 1)
      })
      .on('mouseleave', function() {
        nodeSel.selectAll<SVGCircleElement, SimNode>('circle:first-child').attr('opacity', 1)
        linkSel.attr('stroke-opacity', 0.5)
        hoverLabel.attr('opacity', 0)
      })
      .on('click', (e, d) => {
        e.stopPropagation()
        onNodeClick(d)
      })
      .on('dblclick', (e, d) => {
        e.stopPropagation()
        onNodeDblClick(d)
      })

    // Drag
    nodeSel.call(
      d3.drag<SVGGElement, SimNode>()
        .on('start', (e, d) => {
          if (!e.active) sim.alphaTarget(0.3).restart()
          d.fx = d.x; d.fy = d.y
        })
        .on('drag', (e, d) => { d.fx = e.x; d.fy = e.y })
        .on('end', (e) => { if (!e.active) sim.alphaTarget(0) })
    )

    // Simulation
    const sim = d3.forceSimulation<SimNode>(nodes)
      .force('link', d3.forceLink<SimNode, SimLink>(links)
        .id(d => d.id)
        .distance(80)
        .strength(0.4)
      )
      .force('charge', d3.forceManyBody().strength(-180))
      .force('center', d3.forceCenter(W / 2, H / 2))
      .force('collision', d3.forceCollide<SimNode>(d => nodeRadius(d.link_count) + 3))

    simRef.current = sim

    let frame = 0
    sim.on('tick', () => {
      // Cap to ~60fps
      if (++frame % 2 !== 0) return
      linkSel
        .attr('x1', d => d.source.x)
        .attr('y1', d => d.source.y)
        .attr('x2', d => d.target.x)
        .attr('y2', d => d.target.y)
      nodeSel.attr('transform', d => `translate(${d.x},${d.y})`)
    })

    // Inject pulse animation CSS
    const style = document.createElement('style')
    style.textContent = `
      @keyframes pulse { 0%,100%{opacity:.5;transform:scale(1)} 50%{opacity:.1;transform:scale(1.5)} }
      .pulse-ring { animation: pulse 2s ease-in-out infinite; transform-box: fill-box; transform-origin: center; }
    `
    container.appendChild(style)

    return () => {
      sim.stop()
      d3.select(container).select('svg').remove()
      style.remove()
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nodes, links, showLabels])

  return simRef
}

// ── Canvas renderer (> 150 nodes) ────────────────────────────────────────────

function useCanvasGraph(
  containerRef: React.RefObject<HTMLDivElement | null>,
  nodes: SimNode[],
  links: SimLink[],
  showLabels: boolean,
  onNodeClick: (n: SimNode) => void,
  onNodeDblClick: (n: SimNode) => void,
  onBackgroundDblClick: () => void,
) {
  const simRef = useRef<d3.Simulation<SimNode, SimLink> | null>(null)

  useEffect(() => {
    if (!containerRef.current || nodes.length === 0) return
    const container = containerRef.current
    const W = container.clientWidth
    const H = container.clientHeight
    const PR = window.devicePixelRatio || 1

    // Clean up
    d3.select(container).selectAll('canvas').remove()
    simRef.current?.stop()

    const canvas = document.createElement('canvas')
    canvas.style.cssText = 'position:absolute;inset:0;width:100%;height:100%'
    canvas.width = W * PR
    canvas.height = H * PR
    container.appendChild(canvas)
    const ctx = canvas.getContext('2d')!
    ctx.scale(PR, PR)

    let transform = d3.zoomIdentity

    const zoom = d3.zoom<HTMLCanvasElement, unknown>()
      .scaleExtent([0.2, 4])
      .on('zoom', e => { transform = e.transform; draw() })

    d3.select(canvas).call(zoom)

    let hoveredId: string | null = null

    function getNodeAt(mx: number, my: number): SimNode | null {
      const [wx, wy] = transform.invert([mx, my])
      for (let i = nodes.length - 1; i >= 0; i--) {
        const n = nodes[i]
        const r = nodeRadius(n.link_count)
        if ((n.x - wx) ** 2 + (n.y - wy) ** 2 <= r * r) return n
      }
      return null
    }

    canvas.addEventListener('mousemove', e => {
      const rect = canvas.getBoundingClientRect()
      const n = getNodeAt(e.clientX - rect.left, e.clientY - rect.top)
      const id = n?.id ?? null
      if (id !== hoveredId) { hoveredId = id; draw() }
    })
    canvas.addEventListener('click', e => {
      const rect = canvas.getBoundingClientRect()
      const n = getNodeAt(e.clientX - rect.left, e.clientY - rect.top)
      if (n) onNodeClick(n)
    })
    canvas.addEventListener('dblclick', e => {
      const rect = canvas.getBoundingClientRect()
      const n = getNodeAt(e.clientX - rect.left, e.clientY - rect.top)
      if (n) onNodeDblClick(n)
      else onBackgroundDblClick()
    })

    // Drag on canvas
    d3.select(canvas).call(
      d3.drag<HTMLCanvasElement, unknown>()
        .subject(e => {
          const rect = canvas.getBoundingClientRect()
          return getNodeAt(e.sourceEvent.clientX - rect.left, e.sourceEvent.clientY - rect.top) as unknown as d3.SubjectPosition
        })
        .on('start', (e) => {
          const n = e.subject as unknown as SimNode
          if (!n) return
          if (!e.active) sim.alphaTarget(0.3).restart()
          n.fx = n.x; n.fy = n.y
        })
        .on('drag', (e) => {
          const n = e.subject as unknown as SimNode
          if (!n) return
          n.fx = transform.invertX(e.x)
          n.fy = transform.invertY(e.y)
        })
        .on('end', (e) => {
          if (!e.active) sim.alphaTarget(0)
        })
    )

    function draw() {
      ctx.save()
      ctx.clearRect(0, 0, W, H)
      ctx.translate(transform.x, transform.y)
      ctx.scale(transform.k, transform.k)

      const hovNeighbors = new Set<string>()
      if (hoveredId) {
        hovNeighbors.add(hoveredId)
        links.forEach(l => {
          if (l.source.id === hoveredId) hovNeighbors.add(l.target.id)
          if (l.target.id === hoveredId) hovNeighbors.add(l.source.id)
        })
      }

      // Draw links
      links.forEach(l => {
        const dimmed = hoveredId && !(hovNeighbors.has(l.source.id) && hovNeighbors.has(l.target.id))
        ctx.globalAlpha = dimmed ? 0.04 : 0.45
        ctx.strokeStyle = LINK_COLORS[l.rel_type] ?? DEFAULT_LINK_COLOR
        ctx.lineWidth = 1
        if (l.expired) ctx.setLineDash([4, 3])
        else ctx.setLineDash([])
        ctx.beginPath()
        ctx.moveTo(l.source.x, l.source.y)
        ctx.lineTo(l.target.x, l.target.y)
        ctx.stroke()
      })
      ctx.setLineDash([])

      // Draw nodes
      nodes.forEach(n => {
        const r = nodeRadius(n.link_count)
        const dimmed = hoveredId && !hovNeighbors.has(n.id)
        ctx.globalAlpha = dimmed ? DIM_OPACITY : 1
        ctx.fillStyle = nodeColor(n.primary_tag)
        ctx.beginPath()
        ctx.arc(n.x, n.y, r, 0, Math.PI * 2)
        ctx.fill()

        // Label
        if (showLabels || n.link_count > 3 || n.id === hoveredId) {
          ctx.globalAlpha = dimmed ? 0.1 : 0.85
          ctx.fillStyle = '#c8cbc6'
          ctx.font = '10px IBM Plex Sans, sans-serif'
          ctx.textBaseline = 'middle'
          const label = n.title.length > 22 ? n.title.slice(0, 20) + '…' : n.title
          ctx.fillText(label, n.x + r + 3, n.y)
        }
      })

      ctx.globalAlpha = 1
      ctx.restore()
    }

    const sim = d3.forceSimulation<SimNode>(nodes)
      .force('link', d3.forceLink<SimNode, SimLink>(links).id(d => d.id).distance(70).strength(0.4))
      .force('charge', d3.forceManyBody().strength(-120))
      .force('center', d3.forceCenter(W / 2, H / 2))
      .force('collision', d3.forceCollide<SimNode>(d => nodeRadius(d.link_count) + 2))

    simRef.current = sim

    let raf = 0
    sim.on('tick', () => {
      cancelAnimationFrame(raf)
      raf = requestAnimationFrame(draw)
    })

    return () => {
      cancelAnimationFrame(raf)
      sim.stop()
      canvas.remove()
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nodes, links, showLabels])

  return simRef
}

// ── Side panel ───────────────────────────────────────────────────────────────

function SidePanel({
  node,
  onClose,
  onOpen,
}: {
  node: SimNode
  onClose: () => void
  onOpen: (id: string) => void
}) {
  return (
    <div
      style={{
        position: 'absolute',
        top: 0,
        right: 0,
        width: '250px',
        height: '100%',
        background: 'var(--surface)',
        borderLeft: '1px solid var(--border)',
        display: 'flex',
        flexDirection: 'column',
        zIndex: 10,
        overflow: 'auto',
      }}
    >
      <div style={{ padding: '16px', borderBottom: '1px solid var(--border)', display: 'flex', alignItems: 'flex-start', gap: '8px' }}>
        <div style={{ flex: 1 }}>
          <div
            style={{
              width: 10,
              height: 10,
              borderRadius: '50%',
              background: nodeColor(node.primary_tag),
              display: 'inline-block',
              marginRight: 8,
              verticalAlign: 'middle',
            }}
          />
          <span style={{ fontFamily: 'var(--font-serif)', fontSize: '15px', fontWeight: 600, color: 'var(--text)' }}>
            {node.title}
          </span>
        </div>
        <button
          onClick={onClose}
          aria-label="Close panel"
          style={{ background: 'none', border: 'none', color: 'var(--text3)', cursor: 'pointer', fontSize: '16px', padding: '0 4px' }}
        >
          ×
        </button>
      </div>
      <div style={{ padding: '12px 16px', flex: 1 }}>
        <div style={{ display: 'flex', gap: '16px', marginBottom: '16px', fontFamily: 'var(--font-mono)', fontSize: '11px', color: 'var(--text3)' }}>
          <span>{node.link_count} links</span>
          <span>{node.tag_count} tags</span>
        </div>
        <div style={{ fontFamily: 'var(--font-mono)', fontSize: '10px', color: 'var(--text3)', marginBottom: '8px' }}>
          {node.id.slice(0, 8)}…
        </div>
        <div style={{ fontFamily: 'var(--font-mono)', fontSize: '10px', color: 'var(--text3)' }}>
          created {new Date(node.created_at).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })}
        </div>
        {node.primary_tag && (
          <div style={{ marginTop: '12px' }}>
            <span
              style={{
                padding: '2px 8px',
                borderRadius: '999px',
                background: 'rgba(110,231,160,0.1)',
                border: '1px solid var(--border)',
                fontFamily: 'var(--font-mono)',
                fontSize: '11px',
                color: 'var(--accent)',
              }}
            >
              #{node.primary_tag}
            </span>
          </div>
        )}
      </div>
      <div style={{ padding: '12px 16px', borderTop: '1px solid var(--border)' }}>
        <button
          onClick={() => onOpen(node.id)}
          style={{
            width: '100%',
            padding: '8px',
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
          Open note →
        </button>
      </div>
    </div>
  )
}

// ── Main view ────────────────────────────────────────────────────────────────

export default function GraphView() {
  const navigate = useNavigate()
  const containerRef = useRef<HTMLDivElement>(null)
  const [filterQ, setFilterQ] = useState('')
  const [debouncedQ, setDebouncedQ] = useState('')
  const [showLabels, setShowLabels] = useState(false)
  const [selectedNode, setSelectedNode] = useState<SimNode | null>(null)
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [activeTags, setActiveTags] = useState<Set<string>>(new Set(Object.keys(TAG_COLORS)))
  const [resetKey, setResetKey] = useState(0)

  // Debounce filter
  useEffect(() => {
    const t = setTimeout(() => setDebouncedQ(filterQ), 300)
    return () => clearTimeout(t)
  }, [filterQ])

  const { data, isLoading, isError } = useQuery({
    queryKey: ['graph-full', debouncedQ],
    queryFn: () => client.getFullGraph({ limit: 200, q: debouncedQ || undefined }),
    staleTime: 30_000,
  })

  // Build sim nodes/links
  const { simNodes, simLinks } = (() => {
    if (!data) return { simNodes: [], simLinks: [] }

    const now = Date.now()
    const filteredNodes = data.nodes.filter(n => {
      const tag = n.primary_tag
      if (tag && !activeTags.has(tag)) return false
      if (!tag && !activeTags.has('__none__')) return false
      return true
    })
    const idSet = new Set(filteredNodes.map(n => n.id))

    const simNodes: SimNode[] = filteredNodes.map(n => ({
      ...n,
      x: Math.random() * 800,
      y: Math.random() * 600,
      fx: null,
      fy: null,
    }))

    const simLinks: SimLink[] = data.links
      .filter(l => idSet.has(l.source) && idSet.has(l.target))
      .map(l => {
        const expired = l.valid_until ? new Date(l.valid_until).getTime() < now : false
        return {
          source: simNodes.find(n => n.id === l.source)!,
          target: simNodes.find(n => n.id === l.target)!,
          rel_type: l.rel_type,
          valid_from: l.valid_from,
          valid_until: l.valid_until,
          expired,
        }
      })
      .filter(l => l.source && l.target)

    return { simNodes, simLinks }
  })()

  const useCanvas = simNodes.length > 150

  const handleNodeClick = useCallback((n: SimNode) => {
    setSelectedNode(n)
  }, [])

  const handleNodeDblClick = useCallback((n: SimNode) => {
    navigate(`/notes/${n.id}`)
  }, [navigate])

  const handleBackgroundDblClick = useCallback(() => {
    simNodes.forEach(n => { n.fx = null; n.fy = null })
    setSelectedIds(new Set())
    setSelectedNode(null)
  }, [simNodes])

  const resetKey2 = resetKey // eslint-disable-line

  // Only one of these actually renders; the other is a no-op since the key check is inside
  const svgSimRef = useSvgGraph(
    containerRef,
    useCanvas ? [] : simNodes,
    useCanvas ? [] : simLinks,
    showLabels,
    handleNodeClick,
    handleNodeDblClick,
    handleBackgroundDblClick,
    selectedIds,
  )
  const canvasSimRef = useCanvasGraph(
    containerRef,
    useCanvas ? simNodes : [],
    useCanvas ? simLinks : [],
    showLabels,
    handleNodeClick,
    handleNodeDblClick,
    handleBackgroundDblClick,
  )
  void svgSimRef; void canvasSimRef; void resetKey2

  const allTags = [...Object.keys(TAG_COLORS)]

  const totalVisible = simNodes.length
  const totalAll = data?.total_notes ?? 0

  return (
    <div style={{ position: 'relative', width: '100%', height: '100vh', background: 'var(--bg)', overflow: 'hidden' }}>

      {/* Controls bar */}
      <div
        style={{
          position: 'absolute',
          top: '16px',
          left: '16px',
          zIndex: 20,
          display: 'flex',
          flexDirection: 'column',
          gap: '8px',
          maxWidth: '280px',
        }}
      >
        {/* Search */}
        <div style={{ display: 'flex', gap: '6px', alignItems: 'center' }}>
          <input
            value={filterQ}
            onChange={e => setFilterQ(e.target.value)}
            placeholder="Filter nodes…"
            aria-label="Filter graph nodes"
            style={{
              width: '180px',
              padding: '6px 10px',
              background: 'var(--surface)',
              border: '1px solid var(--border)',
              borderRadius: 'var(--radius-sm)',
              color: 'var(--text)',
              fontSize: '12px',
              fontFamily: 'var(--font-ui)',
            }}
          />
          <button
            onClick={() => { setResetKey(k => k + 1) }}
            title="Reset layout"
            aria-label="Reset layout"
            style={{
              padding: '6px 8px',
              background: 'var(--surface)',
              border: '1px solid var(--border)',
              borderRadius: 'var(--radius-sm)',
              color: 'var(--text3)',
              fontSize: '12px',
              cursor: 'pointer',
            }}
          >
            ↺
          </button>
          <button
            onClick={() => setShowLabels(s => !s)}
            title="Toggle all labels"
            aria-label="Toggle all labels"
            style={{
              padding: '6px 8px',
              background: showLabels ? 'var(--accent)' : 'var(--surface)',
              border: '1px solid var(--border)',
              borderRadius: 'var(--radius-sm)',
              color: showLabels ? 'var(--bg)' : 'var(--text3)',
              fontSize: '11px',
              cursor: 'pointer',
              fontFamily: 'var(--font-mono)',
            }}
          >
            Aa
          </button>
        </div>

        {/* Tag filter chips */}
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '4px' }}>
          {allTags.map(tag => {
            const active = activeTags.has(tag)
            return (
              <button
                key={tag}
                onClick={() => setActiveTags(prev => {
                  const next = new Set(prev)
                  if (next.has(tag)) next.delete(tag)
                  else next.add(tag)
                  return next
                })}
                style={{
                  padding: '2px 7px',
                  borderRadius: '999px',
                  border: `1px solid ${active ? TAG_COLORS[tag] : 'var(--border)'}`,
                  background: active ? `${TAG_COLORS[tag]}20` : 'transparent',
                  color: active ? TAG_COLORS[tag] : 'var(--text3)',
                  fontSize: '10px',
                  fontFamily: 'var(--font-mono)',
                  cursor: 'pointer',
                  transition: 'all 150ms',
                }}
              >
                #{tag}
              </button>
            )
          })}
        </div>

        {/* Node count */}
        <div style={{ fontFamily: 'var(--font-mono)', fontSize: '10px', color: 'var(--text3)' }}>
          {isLoading ? 'Loading…' : isError ? 'Error loading graph' : `Showing ${totalVisible} of ${totalAll} notes`}
        </div>

        {/* Renderer badge */}
        <div style={{ fontFamily: 'var(--font-mono)', fontSize: '10px', color: 'var(--border)' }}>
          {useCanvas ? 'canvas' : 'svg'} · {useCanvas ? 'canvas renderer' : 'svg renderer'}
        </div>
      </div>

      {/* Graph canvas / SVG mount */}
      <div ref={containerRef} style={{ position: 'absolute', inset: 0 }} />

      {/* Loading overlay */}
      {isLoading && (
        <div style={{
          position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center',
          background: 'rgba(13,15,14,0.7)',
        }}>
          <div style={{ color: 'var(--text3)', fontFamily: 'var(--font-mono)', fontSize: '13px' }}>
            Loading graph…
          </div>
        </div>
      )}

      {/* Side panel */}
      {selectedNode && (
        <SidePanel
          node={selectedNode}
          onClose={() => setSelectedNode(null)}
          onOpen={id => navigate(`/notes/${id}`)}
        />
      )}

      {/* Help hint */}
      <div
        style={{
          position: 'absolute',
          bottom: '16px',
          left: '16px',
          fontFamily: 'var(--font-mono)',
          fontSize: '10px',
          color: 'var(--border)',
          pointerEvents: 'none',
        }}
      >
        click · open panel · · dbl-click · open note · · drag · pin · · dbl-click bg · unpin all
      </div>
    </div>
  )
}
