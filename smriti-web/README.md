# smriti-web

React + Vite frontend for the Smriti knowledge graph.

## Setup

```bash
npm install
npm run dev        # dev server at http://localhost:5173 (proxies /api → localhost:3000)
npm run build      # outputs to ../src-tauri/dist/
```

## Environment

| Variable | Default | Description |
|---|---|---|
| `VITE_API_BASE` | `/api/v1` | Smriti REST API base URL |

## Requirements

Smriti backend must be running:
```bash
smriti serve --port 3000
```
