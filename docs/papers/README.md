# Smriti paper drafts

Working directory for the academic paper described in `2026-05-verifiable-agent-memory-outline.md`.

## Source of truth

**Markdown files in this directory are the canonical drafts.** LaTeX files are auto-generated from them. Edit `.md`, never `.tex` directly (except `paper.tex` itself, which is the master template).

## Files

| File | What it is | Status |
|---|---|---|
| `2026-05-verifiable-agent-memory-outline.md` | Paper outline + 6-week drafting plan + venue strategy | Outline complete |
| `2026-05-section-1-introduction.md` | §1 Introduction with FDA-scenario hook | **Drafted** |
| `2026-05-section-2-background.md` | §2 Background: CLS, AGM, FACTUM, graph retrieval | **Drafted** |
| `2026-05-section-3-integrity-contract.md` | §3 Three-invariant formalism (I1/I2/I3) | **Drafted** |
| `2026-05-section-4-architecture.md` | §4 Five-layer stack + compile-time audit boundary | **Drafted** |
| `2026-05-section-6-1-audit-overhead.md` | §6.1 Audit-overhead bench results | **Drafted** (with measured numbers) |
| `paper.tex` | LaTeX master template; `\input{...}`s each section | Skeleton + structure |
| `refs.bib` | Bibliography in BibTeX format | Stubs for the 7 cited works (replace with full entries before submission) |
| `Makefile` | Build pipeline: `.md` → `.tex` (pandoc) → `paper.pdf` (pdflatex) | Ready |
| `sections/*.tex` | Auto-generated section files. Don't edit. | Stubs for undrafted sections |

## Build

```sh
brew install pandoc
brew install --cask mactex-no-gui

cd docs/papers
make sections   # convert drafted .md -> .tex
make            # build paper.pdf
```

## Status (May 2026)

5 of 11 sections drafted. Bench numbers captured for §6.1. Remaining work tracked in the outline's 6-week drafting plan.
