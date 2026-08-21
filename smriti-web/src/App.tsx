import { lazy, Suspense } from 'react'
import { Routes, Route, Navigate } from 'react-router-dom'
import Layout from './components/Layout'
import Dashboard from './views/Dashboard'
import Search from './views/Search'
import NoteDetail from './views/NoteDetail'
import KVStore from './views/KVStore'

// GraphView is lazy to keep the main chunk free of d3 (~280KB)
const GraphView = lazy(() => import('./views/GraphView'))

function GraphFallback() {
  return (
    <div style={{
      display: 'flex', alignItems: 'center', justifyContent: 'center',
      height: '100vh', color: 'var(--text3)', fontFamily: 'var(--font-mono)', fontSize: '13px',
    }}>
      Loading graph…
    </div>
  )
}

export default function App() {
  return (
    <Layout>
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/search" element={<Search />} />
        <Route path="/notes/:id" element={<NoteDetail />} />
        <Route path="/kv" element={<KVStore />} />
        <Route path="/graph" element={
          <Suspense fallback={<GraphFallback />}>
            <GraphView />
          </Suspense>
        } />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </Layout>
  )
}
