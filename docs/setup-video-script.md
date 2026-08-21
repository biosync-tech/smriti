# Smriti Setup Walkthrough — 4-Minute Screen Recording Script

**Goal:** Show a non-technical solo entrepreneur how to go from zero to a working Smriti dashboard in under 5 minutes, tracking their first client.

**Resolution:** 1920×1080. Terminal font size 18px. Browser zoom 110%.

---

## Script

| Timestamp | Screen action | Voiceover |
|-----------|--------------|-----------|
| 0:00–0:05 | Cut straight to the Smriti web dashboard — dashboard view, a few notes visible, the graph panel open | "Here's what we're building: a private knowledge base for your business that your AI assistant can actually read." |
| 0:05–0:15 | Slowly pan across the dashboard — Recent Notes, the graph with colored nodes, the search bar | "Every client, decision, and meeting note lives here — connected, searchable, and completely on your own machine." |
| 0:15–0:30 | Zoom in on the graph view: one node labeled "Acme Corp" linked to "Q2 Strategy Call" and "Sarah Chen" | "When you write a note that references another, Smriti draws the connection automatically. No manual linking." |
| **Install** | | |
| 0:30–0:35 | Switch to a fresh terminal window. Clear the screen. | "Let's install it. You need Rust — if you don't have it, rust-lang.org has a one-line installer." |
| 0:35–0:45 | Type and run: `cargo install smriti`. The download progress begins. | "This downloads and compiles Smriti from source. It takes a minute the first time." |
| 0:45–1:10 | Compilation output scrolls. Cursor blinks. | "Smriti is a single binary — no database server to install, no Docker, no accounts. Your data stays in one file on your machine." |
| 1:10–1:25 | Compilation finishes. Run: `smriti --version` | "When that finishes—" |
| 1:25–1:30 | Output: `smriti 0.1.0` appears | "—you're done. One command." |
| **First note** | | |
| 1:30–1:38 | Run: `smriti new` | "Let's create your first note using the interactive prompt." |
| 1:38–1:48 | Prompt appears: `Title:` — type "Acme Corp" and press Enter | "The title is the name of the thing you're documenting — a client, a project, a decision." |
| 1:48–2:00 | Prompt: `Content:` — type "Large consulting client. Primary contact: [[Sarah Chen]]. Signed [[rel:temporal\|Q1 contract]] in March." | "Notice the double brackets. Those become graph connections — Smriti will link this note to Sarah Chen and the contract automatically." |
| 2:00–2:10 | Prompt: `Tags (space-separated):` — type "client" and press Enter | "Tags let you filter notes later — by client, by project, by decision type." |
| 2:10–2:18 | Prompt: `Link to an existing note? (fuzzy search):` — type "Ac", Acme Corp appears, press Escape | "You can link to an existing note here, or skip it. We'll do that from our next note instead." |
| 2:18–2:25 | Confirmation screen shows the note preview. Press Enter to confirm. Output: `Created: Acme Corp (id: a3f9…)` | "Note created." |
| 2:25–2:30 | Run `smriti new` again. Title: "Q2 Strategy Call". Content: "Discovery call with [[rel:temporal\|Acme Corp]]. Discussed pricing and timeline." Tags: "client meeting". | "A second note — this time with a typed link back to Acme Corp. The `rel:temporal` tag tells the graph this was a time-ordered event." |
| **Web UI** | | |
| 2:30–2:38 | Run: `smriti serve` | "Now start the web interface." |
| 2:38–2:45 | Output shows: `Web UI: http://localhost:3000/`. Open browser to that URL. | "Open your browser to localhost:3000." |
| 2:45–2:58 | Dashboard loads. Point to: Recent Notes section showing "Acme Corp" and "Q2 Strategy Call". Today's focus field. | "The dashboard shows your recent notes and a focus field you can use to tell your AI assistant what you're working on today." |
| 2:58–3:10 | Click "Acme Corp". Note detail opens: title, content, linked notes panel showing "Q2 Strategy Call" and "Sarah Chen" as neighbors. | "Click any note to open it. The linked notes panel on the right shows everything connected to this note — extracted from the text automatically." |
| 3:10–3:22 | Navigate to /graph. The force graph renders: Acme Corp as a larger node, Q2 Strategy Call and Sarah Chen as smaller nodes connected to it. | "The graph view shows the whole picture. Node size reflects how many connections a note has." |
| 3:22–3:30 | Type "strategy" in the search bar at the top of the page. Results appear: "Q2 Strategy Call" at the top. | "Search works instantly across all your notes. No configuration required." |
| **Claude MCP** | | |
| 3:30–3:38 | Switch to Claude.ai. Don't show the login. Show a conversation window already open. | "Now connect it to Claude. The configuration is one JSON block — I'll link it in the description." |
| 3:38–3:48 | Type into the Claude message box: "What do I know about Acme Corp?" Send. | "Ask Claude about your client." |
| 3:48–4:00 | Claude's response appears, summarizing the Acme Corp note: primary contact, the Q2 strategy call, the contract signed in March. | "Claude is reading directly from Smriti. You didn't paste anything. You didn't re-explain the context. It just knows — because you wrote it down once." |

---

## B-roll / cutaway moments

1. **File system close-up** — `ls -lh ~/.local/share/smriti/smriti.db` showing a single small file. Reinforces the "one file, your machine" point made during install.

2. **Graph animation** — The force-directed graph settling from chaos into clusters after nodes are loaded. Shot without narration, 3–4 seconds. Shows the visual payoff before users have context for what they're looking at.

3. **Split screen: paste vs. MCP** — Left: someone copying and pasting a note into a Claude conversation manually. Right: Claude reading from Smriti via MCP with no copying. No narration needed; the contrast is the point.

4. **Terminal benchmark output** — Run `cargo bench` and cut to just the KV retrieval and BFS lines (`2.48 µs`, `235 ns`). For a developer-audience cut of the video.

5. **Mobile browser on localhost:3000** — Show the responsive dashboard on a phone connected to the same local network. Demonstrates "your machine" doesn't mean desktop-only.

---

## Thumbnail

**Text (max 6 words):**

> Your AI's memory. Your machine.

*Alternate:*

> Private AI memory. Zero cloud.
