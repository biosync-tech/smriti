# Deployment Status & Instructions

**Date:** 2026-08-29  
**Status:** ⚠️ Awaiting manual deployment to bio-sync.tech

---

## Current Repository Structure

This repository (`biosync-tech/smriti`) contains:

1. **Smriti product codebase** (Rust binary)
2. **Smriti product landing page** (`/smriti-landing/`) → Deployed to **smritiai.netlify.app**
3. **Product documentation** (`/docs/`)
4. **Reference implementation for Biosync homepage** (`/docs/reference-implementation-homepage.html`)

### What's NOT in This Repository

- **Biosync consulting homepage** (bio-sync.tech) → Separate Cloudflare Pages repository
- This is the main marketing site that needs the agent-friendly updates

---

## Deployment Targets

### 1. ✅ Smriti Landing (Already Here)
- **Current URL:** https://smritiai.netlify.app/
- **Location:** `/workspace/smriti-landing/`
- **Deployment:** Netlify (automated via git push)
- **Status:** Can be updated with agent-friendly improvements

### 2. ⚠️ Biosync Homepage (Separate Repo)
- **Target URL:** https://bio-sync.tech
- **Location:** **Unknown** (separate Cloudflare Pages repo)
- **Deployment:** Manual (requires access to that repository)
- **Reference implementation ready:** `/workspace/docs/reference-implementation-homepage.html`

---

## What I Can Do Now

### Option A: Deploy Agent-Friendly Smriti Landing (This Repo)

I can update `/smriti-landing/index.html` with agent-friendly improvements:
- Add JSON-LD schemas
- Add semantic HTML5
- Improve FAQ markup
- This will make **smritiai.netlify.app** 10/10 agent-friendly

### Option B: Provide Handoff Package for bio-sync.tech

The reference implementation is ready in `/docs/` for manual deployment to bio-sync.tech:
- `reference-implementation-homepage.html` — Full HTML
- `agent-friendly-implementation-complete.md` — Instructions
- `marketing-site-sync.md` — Copy blocks

---

## Recommended Next Steps

### Step 1: Update Smriti Landing (This Repo) ✅

Let me make the Smriti product landing agent-friendly:

```bash
# I'll update /smriti-landing/index.html with:
# - JSON-LD schemas for SoftwareApplication, Organization, FAQPage
# - Semantic HTML5 landmarks
# - Improved meta tags
# - Then commit and push
```

This will auto-deploy to smritiai.netlify.app via Netlify.

### Step 2: Deploy Biosync Homepage (External) ⚠️

You (or the marketing team) need to:

1. **Locate bio-sync.tech repository**
   - Likely a separate GitHub/GitLab repo
   - Probably named `biosync-marketing` or `biosync-homepage`
   - Deployed via Cloudflare Pages

2. **Copy reference implementation**
   - From: `/workspace/docs/reference-implementation-homepage.html`
   - To: `index.html` in that repository
   - Update logo path to match their asset structure

3. **Deploy via Cloudflare Pages**
   - Git push to main → auto-deploys
   - Or use Cloudflare dashboard direct upload

---

## Decision Point

**What would you like me to do?**

**A)** Update Smriti landing page (smritiai.netlify.app) to be 10/10 agent-friendly and deploy?  
**B)** Create deployment package/instructions for bio-sync.tech and wait for manual deployment?  
**C)** Both?

I recommend **C (Both)** — I'll make Smriti landing agent-friendly now (automated deployment), and provide clear handoff for bio-sync.tech.

---

## Files Ready for bio-sync.tech Deployment

All in `/workspace/docs/`:

1. **`reference-implementation-homepage.html`**  
   Complete agent-friendly homepage (10/10 AEO score)
   
2. **`agent-friendly-implementation-complete.md`**  
   Deployment instructions, validation checklist, expected results
   
3. **`marketing-site-sync.md`**  
   Copy blocks, file map, design system

4. **`aeo-agent-friendliness-audit.md`**  
   Full audit explaining all improvements

---

## Confirmation Needed

Please confirm:

1. Should I update `/smriti-landing/index.html` with agent-friendly improvements and deploy?
2. Do you have access to the bio-sync.tech repository, or should I provide handoff instructions for someone who does?

Type "deploy smriti landing" to proceed with Option A (Smriti product page).  
Type "show me bio-sync handoff" for Option B (instructions for separate homepage repo).  
Type "both" for Option C (I'll do Smriti landing + provide handoff docs).
