# =====================================================================
# Smriti paper build
#
# Source of truth: Markdown drafts in this directory.
# Build pipeline: Markdown -> .tex (via pandoc) -> paper.pdf (via pdflatex).
#
# Usage:
#   make            -> build paper.pdf
#   make clean      -> remove generated artifacts
#   make sections   -> only convert .md -> sections/*.tex (no PDF build)
#
# Prerequisites:
#   brew install pandoc
#   brew install --cask mactex-no-gui    # or any TeX distribution
#
# Markdown source -> tex output mapping. Add new rows when sections
# move from outline to drafted.
# =====================================================================

PANDOC      := pandoc
PANDOC_OPTS := --from gfm --to latex --top-level-division=part --wrap=preserve

PDFLATEX    := pdflatex
BIBTEX      := bibtex

# Drafted sections (Markdown source -> tex destination)
DRAFTED_MD := \
  2026-05-section-1-introduction.md           \
  2026-05-section-2-background.md             \
  2026-05-section-3-integrity-contract.md     \
  2026-05-section-4-architecture.md           \
  2026-05-section-6-1-audit-overhead.md

DRAFTED_TEX := \
  sections/01-introduction.tex                \
  sections/02-background.tex                  \
  sections/03-integrity-contract.tex          \
  sections/04-architecture.tex                \
  sections/06-1-audit-overhead.tex

# Default target
all: paper.pdf

# Build PDF (requires all sections to exist as .tex stubs in sections/)
paper.pdf: paper.tex refs.bib $(DRAFTED_TEX) sections/abstract.tex
	@echo "==> pdflatex pass 1"
	$(PDFLATEX) -interaction=nonstopmode paper.tex
	@echo "==> bibtex"
	$(BIBTEX) paper
	@echo "==> pdflatex pass 2"
	$(PDFLATEX) -interaction=nonstopmode paper.tex
	@echo "==> pdflatex pass 3"
	$(PDFLATEX) -interaction=nonstopmode paper.tex

# Convert Markdown drafts to LaTeX
sections: $(DRAFTED_TEX)

sections/01-introduction.tex: 2026-05-section-1-introduction.md
	@mkdir -p sections
	$(PANDOC) $(PANDOC_OPTS) $< -o $@

sections/02-background.tex: 2026-05-section-2-background.md
	@mkdir -p sections
	$(PANDOC) $(PANDOC_OPTS) $< -o $@

sections/03-integrity-contract.tex: 2026-05-section-3-integrity-contract.md
	@mkdir -p sections
	$(PANDOC) $(PANDOC_OPTS) $< -o $@

sections/04-architecture.tex: 2026-05-section-4-architecture.md
	@mkdir -p sections
	$(PANDOC) $(PANDOC_OPTS) $< -o $@

sections/06-1-audit-overhead.tex: 2026-05-section-6-1-audit-overhead.md
	@mkdir -p sections
	$(PANDOC) $(PANDOC_OPTS) $< -o $@

# Remove generated artifacts but keep the source drafts
clean:
	rm -f paper.aux paper.bbl paper.blg paper.log paper.out paper.pdf paper.toc
	# Only remove auto-generated .tex files (the placeholder stubs stay)
	rm -f $(DRAFTED_TEX)

.PHONY: all sections clean
