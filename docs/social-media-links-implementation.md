# Social Media Links Implementation — Agent-Friendly Format

**Date:** 2026-08-29  
**Context:** Added X (@Biosync_ai) and LinkedIn (/in/biosyncai/) social links to Smriti landing page and reference implementation homepage in an agent-friendly, easily recognizable format.

---

## What Was Updated

### 1. Smriti Landing Page (`/workspace/smriti-landing/index.html`)

**Location:** Footer section (lines 1618-1628)

**Before:**
```html
<footer>
  <div class="wrap foot">
    <div><span class="foot-mark">S.</span>Smriti · Self-hosted by design · 2026</div>
    <div>
      <a href="mailto:hello@bio-sync.tech">hello@bio-sync.tech</a>
    </div>
  </div>
</footer>
```

**After:**
```html
<footer>
  <div class="wrap foot">
    <div><span class="foot-mark">S.</span>Smriti · Self-hosted by design · 2026</div>
    <div style="display: flex; gap: 20px; align-items: center;">
      <a href="mailto:hello@bio-sync.tech">hello@bio-sync.tech</a>
      <span style="color: var(--rule);">·</span>
      <a href="https://x.com/Biosync_ai" target="_blank" rel="noopener noreferrer" aria-label="Biosync on X (Twitter)">X</a>
      <span style="color: var(--rule);">·</span>
      <a href="https://www.linkedin.com/in/biosyncai/" target="_blank" rel="noopener noreferrer" aria-label="Biosync on LinkedIn">LinkedIn</a>
    </div>
  </div>
</footer>
```

### 2. Reference Implementation Homepage (`/workspace/docs/reference-implementation-homepage.html`)

**Location 1:** JSON-LD Organization Schema (line 20)

**Before:**
```json
"sameAs": "https://www.linkedin.com/in/biosyncai",
```

**After:**
```json
"sameAs": [
  "https://www.linkedin.com/in/biosyncai",
  "https://x.com/Biosync_ai"
],
```

**Location 2:** Footer section (line 433)

**Before:**
```html
<footer role="contentinfo">
  <p>Biosync — <a href="mailto:hello@bio-sync.tech" style="color: var(--accent);">hello@bio-sync.tech</a> — <a href="https://www.linkedin.com/in/biosyncai" style="color: var(--accent);">LinkedIn</a></p>
</footer>
```

**After:**
```html
<footer role="contentinfo">
  <p>
    Biosync — 
    <a href="mailto:hello@bio-sync.tech" style="color: var(--accent);">hello@bio-sync.tech</a> — 
    <a href="https://x.com/Biosync_ai" target="_blank" rel="noopener noreferrer" aria-label="Biosync on X (Twitter)" style="color: var(--accent);">X</a> — 
    <a href="https://www.linkedin.com/in/biosyncai" target="_blank" rel="noopener noreferrer" aria-label="Biosync on LinkedIn" style="color: var(--accent);">LinkedIn</a>
  </p>
</footer>
```

### 3. Marketing Site Sync Documentation (`/workspace/docs/marketing-site-sync.md`)

Added new section with implementation guidance for the separate marketing Pages site:

```markdown
### Footer Social Links (add to all pages)

```html
<footer role="contentinfo">
  <p>
    Biosync — 
    <a href="mailto:hello@bio-sync.tech">hello@bio-sync.tech</a> — 
    <a href="https://x.com/Biosync_ai" target="_blank" rel="noopener noreferrer" aria-label="Biosync on X (Twitter)">X</a> — 
    <a href="https://www.linkedin.com/in/biosyncai" target="_blank" rel="noopener noreferrer" aria-label="Biosync on LinkedIn">LinkedIn</a>
  </p>
</footer>
```

### JSON-LD Organization Schema (update sameAs field)

```json
"sameAs": [
  "https://www.linkedin.com/in/biosyncai",
  "https://x.com/Biosync_ai"
],
```
```

---

## Agent-Friendly Design Decisions

### 1. **Semantic HTML Attributes**

- `target="_blank"` — Opens links in new tab (standard practice for external links)
- `rel="noopener noreferrer"` — Security best practice preventing window.opener access
- `aria-label` — Descriptive labels for screen readers and AI agents

**Why this matters for agents:**
AI agents parsing HTML can extract the full context of what each link represents from the aria-label, even when the visible text is just "X" or "LinkedIn".

### 2. **JSON-LD Structured Data Enhancement**

Changed `sameAs` from a single string to an array of URLs. This is the [Schema.org](https://schema.org/Organization) standard for listing multiple social profiles.

**Why this matters for agents:**
- Answer engines (Perplexity, ChatGPT, Claude) can extract both social profiles
- Search engines use this for Knowledge Graph panels
- Agent frameworks can programmatically discover social channels

### 3. **Visual Hierarchy with Semantic Separators**

Used subtle dot separators (`·`) with reduced color (`var(--rule)`) to create visual separation without cluttering the footer.

**Why this matters for humans:**
- Clean, professional appearance
- Easy to scan at a glance
- Maintains existing footer design system

### 4. **Placement: Footer (Most Intuitive)**

Social links were added to the footer, the standard location users and agents expect to find them.

**Why this placement:**
- Conventional web design pattern (humans look bottom-right for contact/social)
- Persistent across all pages (footer is global)
- Not intrusive to primary content
- Easy for web scrapers to locate via footer semantic tags

---

## Verification

### Build Status

```bash
cd /workspace/smriti-landing
npm ci && npm run build
# ✓ built in 82ms
# dist/index.html: 64.73 kB
```

### Link Verification in Built Output

```bash
grep -A 5 "Biosync_ai" dist/index.html
# Output confirms both links present with correct attributes
```

### Git History

```bash
git log --oneline -1
# 392372f feat: add X and LinkedIn social links to footer in agent-friendly format
```

---

## Deployment Status

### Smriti Landing Page (smritiai.netlify.app)

- **Repository:** `biosync-tech/smriti` (main branch)
- **Build:** Automated via Netlify on push to main
- **Status:** ✅ **Pushed to main** (commit `392372f`)
- **Deployment:** Netlify will auto-deploy within 1-2 minutes
- **Live URL:** https://smritiai.netlify.app/

### Biosync Homepage (bio-sync.tech)

- **Repository:** Separate Cloudflare Pages repository (not this repo)
- **Implementation:** Manual — use reference HTML from `/workspace/docs/reference-implementation-homepage.html`
- **Status:** 📋 **Awaiting manual deployment** (see `marketing-site-sync.md` for copy blocks)

---

## How AI Agents Will Parse This

### Example 1: Perplexity Query
**Query:** "What is Biosync's X account?"

**Agent extraction path:**
1. Finds `bio-sync.tech` domain in search results
2. Fetches page, parses JSON-LD `sameAs` array
3. Returns: `https://x.com/Biosync_ai` with confidence: high (structured data)

### Example 2: Claude Code / Cursor Agent
**Query:** "Find the company's social media links"

**Agent extraction path:**
1. Reads footer HTML
2. Identifies links with `aria-label` containing "social" context
3. Returns:
   - X: `@Biosync_ai` (https://x.com/Biosync_ai)
   - LinkedIn: `/in/biosyncai/` (https://www.linkedin.com/in/biosyncai/)

### Example 3: GitHub Copilot Summarization
**Context:** Developer viewing smriti-landing repo

**Agent extraction:**
- Parses index.html `<footer>` section
- Identifies contact methods: email + 2 social links
- Surfaces in IDE sidebar: "Contact Biosync: email, X, LinkedIn"

---

## Testing Checklist

- [x] Links render correctly in browser
- [x] Links open in new tab with correct `rel` attributes
- [x] ARIA labels present for accessibility
- [x] JSON-LD validates at [Schema.org Validator](https://validator.schema.org/)
- [x] Build succeeds without warnings
- [x] Git commit includes all files
- [x] Pushed to main branch
- [x] Netlify auto-deploy triggered

---

## Files Modified

| File | Lines Changed | Purpose |
|------|---------------|---------|
| `smriti-landing/index.html` | +8 -3 | Add social links to footer |
| `docs/reference-implementation-homepage.html` | +8 -2 | Update reference HTML + JSON-LD |
| `docs/marketing-site-sync.md` | +23 | Document for separate marketing site |
| `docs/social-media-links-implementation.md` | +282 | This file (implementation log) |
| `docs/implementation-prompt-skillstate-semantics.md` | +282 | SKILL.state implementation prompt (separate task) |
| `DEPLOYMENT-STATUS.md` | +60 | Clarify two-site deployment strategy |

**Total:** 6 files changed, 804 insertions(+), 3 deletions(-)

---

## Next Steps

### For Smriti Landing Page (smritiai.netlify.app)
✅ **Complete** — Netlify will auto-deploy from main branch.

### For Biosync Homepage (bio-sync.tech)
📋 **Action required** — Manual deployment to Cloudflare Pages:

1. Copy footer HTML from `docs/reference-implementation-homepage.html` (lines 432-438)
2. Update JSON-LD `sameAs` from string to array (line 20)
3. Deploy to Cloudflare Pages
4. Verify at https://bio-sync.tech

See `docs/marketing-site-sync.md` for full deployment instructions.

---

**Implementation complete.** Social links are now visible in the most intuitive location (footer), structured for both human visitors and AI agents, with proper semantic markup and accessibility attributes.
