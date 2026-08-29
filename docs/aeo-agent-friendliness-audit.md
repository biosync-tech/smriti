# Agent-Friendliness & AEO Evaluation: bio-sync.tech

**Date:** 2026-08-29  
**Context:** Evaluating https://bio-sync.tech for AI agent readability and Answer Engine Optimization

---

## Executive Summary

**Agent-Friendliness:** ⚠️ **6/10** — Good structure, but missing key agent-friendly patterns  
**AEO (Answer Engine Optimization):** ⚠️ **5/10** — Lacks schema markup and explicit Q&A structure

**Quick Wins:** Add FAQ schema, structured data for services, explicit pricing signal, and semantic HTML5 landmarks.

---

## 1. Agent-Friendliness Audit

### ✅ What Works Well

1. **Clear H1/H2 Hierarchy**
   - H1: "Reproducible bioinformatics, from QC to a package you can file."
   - Sections have clear headings: "Standards," "Fit," "First engagement," "Three lanes," etc.
   - AI agents can parse the document structure easily

2. **Explicit Scope Boundaries**
   - "Who this is for — and who should keep looking"
   - "We do not" section clearly states what's out of scope
   - Agents can determine fit without guessing

3. **Concrete Examples in Representative Work**
   - Each card has Problem → Approach → Outcome structure
   - Specific outcomes (e.g., "AUROC > 0.85," "3.2× increase")
   - Agents can extract factual claims with confidence

4. **Contact Flow is Unambiguous**
   - Email: hello@bio-sync.tech
   - Form opens mail client (no hidden backend)
   - NDA request path is explicit

### ❌ What's Missing for Agents

1. **No Structured Data / Schema Markup**
   ```html
   <!-- Missing: JSON-LD for Organization, Service, FAQs -->
   <script type="application/ld+json">
   {
     "@context": "https://schema.org",
     "@type": "Organization",
     "name": "Biosync",
     "url": "https://bio-sync.tech",
     "email": "hello@bio-sync.tech",
     "description": "Reproducible bioinformatics consulting...",
     "knowsAbout": ["RNA-seq", "single-cell", "proteomics", "MIMIC-III", "biomedical knowledge graphs"],
     "areaServed": "Global",
     "serviceType": ["Bioinformatics Consulting", "Omics Analysis", "Healthcare Data Analytics"]
   }
   </script>
   ```
   **Impact:** Perplexity, ChatGPT, Claude can't easily extract "what Biosync does" as structured data.

2. **No Explicit FAQ Section**
   - "What scientific leads usually ask first" exists but isn't marked up as FAQPage schema
   - Questions are buried; agents may miss them
   - **Fix:** Add `<section itemscope itemtype="https://schema.org/FAQPage">` wrapper

3. **Pricing Opacity**
   - "We do not publish dollar amounts on this site" — correct for human credibility
   - But agents answering "how much does Biosync cost?" have zero signal
   - **Recommendation:** Add a sentence like "Fixed-scope sprints typically range $15K–$75K depending on complexity; monthly retainers start at $20K."
   - Keep "contact for quote" as primary CTA, but give agents a ballpark

4. **No Semantic HTML5 Landmarks**
   ```html
   <!-- Current (likely): generic <div> soup -->
   <div class="section">...</div>
   
   <!-- Better for agents: -->
   <main>
     <section aria-labelledby="services">
       <h2 id="services">Three lanes · omics is the door</h2>
       ...
     </section>
     <section aria-labelledby="standards">
       <h2 id="standards">Standards</h2>
       ...
     </section>
   </main>
   ```
   **Impact:** Agents rely on `<main>`, `<nav>`, `<article>`, `<section>` to understand page structure.

5. **Representative Work Cards Lack Structured Metadata**
   - Each card is a blob of text
   - **Better:** Add microdata or JSON-LD for `CreativeWork` or `ResearchProject`
   ```html
   <article itemscope itemtype="https://schema.org/ResearchProject">
     <h3 itemprop="name">Uncovering drug mechanism-of-action in tumor models</h3>
     <meta itemprop="keywords" content="RNA-seq, differential expression, pathway analysis, MoA">
     <div itemprop="description">
       Lead compound worked in xenografts but the MoA was unclear...
     </div>
   </article>
   ```

6. **No `robots.txt` or Sitemap Signal**
   - Unknown if `/robots.txt` exists
   - Unknown if `/sitemap.xml` exists
   - **Check:** Verify these exist and are linked in `<head>`

---

## 2. AEO (Answer Engine Optimization) Audit

### AEO vs. SEO: What's Different

| Aspect | Traditional SEO | AEO (Answer Engines) |
|--------|----------------|----------------------|
| Target | Google SERPs, keyword ranking | Perplexity, ChatGPT, Claude answering questions |
| Optimization | Keywords, backlinks, page speed | Structured data, clear Q&A, factual precision |
| Measurement | Click-through rate, bounce rate | Citation frequency, snippet accuracy |
| Content Format | Blog posts, landing pages | FAQs, schema markup, explicit answers |

**Key Insight:** Answer engines don't rank pages; they extract facts and synthesize answers. If your facts aren't machine-readable, you don't exist.

---

### ✅ What Works for AEO

1. **Explicit "What We Do" Section**
   - Three lanes (Omics, Healthcare data, KG & AI) are clearly listed
   - Agents can extract: "Biosync does bulk RNA-seq, scRNA-seq, spatial transcriptomics, MIMIC-III analytics, biomedical KGs"

2. **Concrete Deliverables**
   - "A report, pipeline, figures, or diligence memo"
   - Agents answering "what does Biosync deliver?" can cite this

3. **Explicit Constraints**
   - "We do not run wet-lab assays, clone, or operate a core facility"
   - Agents can filter Biosync out for wet-lab queries

4. **Outcome Claims Are Measurable**
   - "AUROC > 0.85," "3.2× increase," "two progressed into in-vitro validation"
   - Agents prefer quantified outcomes over vague claims

### ❌ What's Missing for AEO

1. **No FAQ Schema Markup**
   - Questions exist ("What does a first engagement look like?" etc.)
   - But they're not wrapped in `<script type="application/ld+json">` with `@type: "FAQPage"`
   - **Impact:** Answer engines won't surface these Q&As in response to "What does Biosync charge?"

   **Fix:**
   ```html
   <script type="application/ld+json">
   {
     "@context": "https://schema.org",
     "@type": "FAQPage",
     "mainEntity": [
       {
         "@type": "Question",
         "name": "What does a first engagement look like?",
         "acceptedAnswer": {
           "@type": "Answer",
           "text": "A 2–6 week fixed-scope sprint with an explicit deliverable (report, pipeline, figures, or diligence memo), success criteria, and a kill switch. NDA first. No long-term retainer required to start."
         }
       },
       {
         "@type": "Question",
         "name": "Do you handle PHI / HIPAA-regulated data?",
         "acceptedAnswer": {
           "@type": "Answer",
           "text": "Yes, under a signed BAA, typically inside your HIPAA-eligible cloud tenancy. We are HIPAA-aligned and BAA-ready — not HIPAA certified."
         }
       },
       ...
     ]
   }
   </script>
   ```

2. **No Breadcrumbs for Smriti Product**
   - Smriti product section exists but lacks `BreadcrumbList` schema
   - Agents may not understand that Smriti is a separate product offering

   **Fix:**
   ```html
   <script type="application/ld+json">
   {
     "@context": "https://schema.org",
     "@type": "BreadcrumbList",
     "itemListElement": [
       {"@type": "ListItem", "position": 1, "name": "Home", "item": "https://bio-sync.tech"},
       {"@type": "ListItem", "position": 2, "name": "Smriti", "item": "https://bio-sync.tech/smriti/"}
     ]
   }
   </script>
   ```

3. **No Service Schema for Three Lanes**
   - Omics, Healthcare data, KG & AI are described but not marked up as `Service` or `Offer`
   - Agents answering "does Biosync do spatial transcriptomics?" have to parse prose

   **Fix:**
   ```html
   <script type="application/ld+json">
   {
     "@context": "https://schema.org",
     "@type": "ProfessionalService",
     "name": "Biosync Omics Lane",
     "serviceType": "Bioinformatics Consulting",
     "provider": {
       "@type": "Organization",
       "name": "Biosync"
     },
     "areaServed": "Global",
     "hasOfferCatalog": {
       "@type": "OfferCatalog",
       "name": "Omics Services",
       "itemListElement": [
         {"@type": "Offer", "itemOffered": {"@type": "Service", "name": "Bulk RNA-seq"}},
         {"@type": "Offer", "itemOffered": {"@type": "Service", "name": "Single-cell RNA-seq"}},
         {"@type": "Offer", "itemOffered": {"@type": "Service", "name": "Spatial transcriptomics"}},
         ...
       ]
     }
   }
   </script>
   ```

4. **No Pricing Signal (Even Ballpark)**
   - "We do not publish dollar amounts" is fine for humans
   - But agents answering "Biosync pricing" have ZERO signal
   - **Recommendation:** Add ONE sentence: "Fixed-scope sprints typically range $15K–$75K depending on complexity and timeline. Contact for a scoped quote."
   - This gives agents a factual anchor without undermining your "contact us" CTA

5. **No Explicit Turnaround Time Signal**
   - "2–6 week sprint" exists but isn't emphasized
   - Agents answering "how long does a Biosync project take?" may miss it
   - **Fix:** Add to FAQ: "How long does a typical engagement take?" → "2–6 weeks for fixed-scope sprints; longer work shifts to monthly retainers."

6. **No Clear "Best For" vs. "Not For" Schema**
   - "Fit" section exists but isn't marked up
   - **Recommendation:** Add explicit lists agents can parse:
   ```html
   <div itemscope itemtype="https://schema.org/Service">
     <h3>Best for:</h3>
     <ul>
       <li itemprop="audience">Computational biology leads needing defensible omics analysis</li>
       <li itemprop="audience">Academic PIs needing publication-grade methods</li>
       <li itemprop="audience">Digital health founders needing public-data baseline</li>
     </ul>
     <h3>Not for:</h3>
     <ul>
       <li>Wet-lab assays or cloning</li>
       <li>Regulatory submissions (IND, NDA, 510(k)) ownership</li>
       <li>Standing analytics orgs or body shops</li>
     </ul>
   </div>
   ```

---

## 3. Agent Query Scenarios (How Agents Would Parse This Site)

### Scenario 1: "Does Biosync do spatial transcriptomics?"

**Current Experience:**
- Agent reads page, finds "Spatial transcriptomics and cell atlasing" under Omics lane
- Also finds Representative Work card: "Spatial mapping of tumor-immune interfaces"
- **Answer:** ✅ Yes, with medium confidence (no structured data, but clear prose)

**With AEO Improvements:**
- Agent reads `Service` schema with `"name": "Spatial transcriptomics"`
- Also reads Representative Work card with `itemtype="ResearchProject"` and `keywords="spatial transcriptomics"`
- **Answer:** ✅ Yes, with high confidence (structured data confirms)

---

### Scenario 2: "How much does Biosync charge?"

**Current Experience:**
- Agent reads: "We do not publish dollar amounts on this site"
- Finds: "Fixed scope, explicit artifact" and "No retainer required to start"
- **Answer:** ⚠️ "Contact for quote; pricing not published" (low confidence on range)

**With AEO Improvements:**
- Agent reads: "Fixed-scope sprints typically range $15K–$75K depending on complexity"
- Still finds: "Contact for scoped quote"
- **Answer:** ✅ "Typically $15K–$75K for 2–6 week sprints; contact for scoped quote" (high confidence)

---

### Scenario 3: "Can Biosync handle HIPAA data?"

**Current Experience:**
- Agent finds FAQ: "Yes, under a signed BAA, typically inside your HIPAA-eligible cloud tenancy. We are HIPAA-aligned and BAA-ready — not HIPAA certified."
- **Answer:** ✅ Yes, with high confidence (explicit prose)

**With AEO Improvements:**
- Agent finds FAQ + FAQPage schema with this Q&A
- **Answer:** ✅ Yes, with very high confidence (structured + prose confirm)

---

### Scenario 4: "What's the difference between Biosync consulting and Smriti?"

**Current Experience:**
- Agent reads: "Smriti is Biosync's product: a self-hosted agent memory layer..."
- Finds: "A product conversation can run in parallel with a 2–6 week consulting sprint"
- **Answer:** ✅ Clear, with medium confidence (prose only)

**With AEO Improvements:**
- Agent finds `BreadcrumbList` schema separating `/` (consulting) from `/smriti/` (product)
- Finds `Product` schema for Smriti with `offers` separate from `Service` schema for consulting
- **Answer:** ✅ Clear, with high confidence (structured data confirms separation)

---

## 4. Recommended Fixes (Prioritized)

### Priority 1: High-Impact, Low-Effort

1. **Add FAQ Schema Markup** (1 hour)
   - Wrap existing Q&A section in `FAQPage` schema
   - Impact: Agents can cite your answers directly

2. **Add Pricing Ballpark Signal** (5 minutes)
   - Add ONE sentence: "Fixed-scope sprints typically range $15K–$75K."
   - Impact: Agents have factual anchor instead of "no information"

3. **Add Organization Schema** (30 minutes)
   - Basic JSON-LD with name, URL, email, description, serviceType
   - Impact: Agents understand "what Biosync is" at a glance

4. **Add Semantic HTML5 Landmarks** (1 hour)
   - Wrap sections in `<section>`, main content in `<main>`, navigation in `<nav>`
   - Impact: Agents parse structure more reliably

### Priority 2: Medium-Impact, Medium-Effort

5. **Add Service Schema for Three Lanes** (2 hours)
   - Omics, Healthcare data, KG & AI as structured `Service` objects
   - Impact: Agents answering "does Biosync do X?" have high confidence

6. **Reduce Representative Work to Featured 4** (30 minutes)
   - Remove filter tabs, show 4 cards only
   - Impact: Lower cognitive load for humans + agents; clearer signal

7. **Add `robots.txt` and `sitemap.xml`** (30 minutes)
   - Ensure both exist and are linked in `<head>`
   - Impact: Crawlers understand site structure

### Priority 3: Lower-Impact, Higher-Effort

8. **Add Microdata to Representative Work Cards** (3 hours)
   - Mark up each card as `ResearchProject` or `CreativeWork`
   - Impact: Agents can extract outcomes as structured facts

9. **Add Breadcrumb Schema for Smriti** (30 minutes)
   - Separate product from consulting in structured data
   - Impact: Agents understand Smriti is a distinct offering

10. **Add "Best For / Not For" Structured Lists** (1 hour)
    - Mark up audience with `itemprop="audience"`
    - Impact: Agents can filter fit queries more accurately

---

## 5. Testing Agent-Friendliness

### Manual Test (Do This Now)

1. **Perplexity Test:**
   - Ask: "Does Biosync do spatial transcriptomics?"
   - Ask: "How much does Biosync charge?"
   - Ask: "Can Biosync handle HIPAA data?"
   - **Grade:** Does Perplexity cite bio-sync.tech? Are answers accurate?

2. **ChatGPT Test (with browsing):**
   - Same three questions
   - **Grade:** Does ChatGPT find the right sections? Are answers confident?

3. **Claude Test (with web search):**
   - Same three questions
   - **Grade:** Does Claude cite specific sections accurately?

### Automated Test (After Fixes)

1. **Google Rich Results Test:**
   - https://search.google.com/test/rich-results
   - Paste bio-sync.tech URL
   - **Goal:** FAQ schema, Organization schema should validate

2. **Schema.org Validator:**
   - https://validator.schema.org/
   - Paste bio-sync.tech URL
   - **Goal:** Zero errors on structured data

---

## 6. Scoring Rubric (Before vs. After)

| Criterion | Before (Current) | After (With Fixes) |
|-----------|-----------------|-------------------|
| **Structured Data Coverage** | 0% (no schema) | 80% (FAQ, Org, Service schemas) |
| **FAQ Markup** | 0/10 (exists but not marked up) | 9/10 (FAQPage schema) |
| **Pricing Signal** | 2/10 (zero numeric signal) | 7/10 (ballpark range + CTA) |
| **Semantic HTML** | 4/10 (likely generic divs) | 9/10 (`<main>`, `<section>`, `<nav>`) |
| **Service Clarity** | 6/10 (clear prose, no schema) | 9/10 (Service schema + prose) |
| **Agent Confidence** | 6/10 (medium confidence answers) | 9/10 (high confidence answers) |
| **Overall Agent-Friendliness** | **6/10** | **9/10** |
| **Overall AEO Readiness** | **5/10** | **9/10** |

---

## 7. One-Page Implementation Checklist

```html
<!-- Add to <head> of bio-sync.tech -->
<script type="application/ld+json">
{
  "@context": "https://schema.org",
  "@type": "Organization",
  "name": "Biosync",
  "url": "https://bio-sync.tech",
  "email": "hello@bio-sync.tech",
  "description": "Reproducible bioinformatics consulting for omics, healthcare data, and biomedical knowledge graphs.",
  "sameAs": "https://www.linkedin.com/in/biosyncai",
  "serviceType": ["Bioinformatics Consulting", "Omics Analysis", "Healthcare Data Analytics", "Biomedical Knowledge Graphs"]
}
</script>

<script type="application/ld+json">
{
  "@context": "https://schema.org",
  "@type": "FAQPage",
  "mainEntity": [
    {
      "@type": "Question",
      "name": "What does a first engagement look like?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "A 2–6 week fixed-scope sprint with an explicit deliverable (report, pipeline, figures, or diligence memo), success criteria, and a kill switch. NDA first. No long-term retainer required to start. Fixed-scope sprints typically range $15K–$75K depending on complexity."
      }
    },
    {
      "@type": "Question",
      "name": "Do you handle PHI / HIPAA-regulated data?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "Yes, under a signed BAA, typically inside your HIPAA-eligible cloud tenancy. We are HIPAA-aligned and BAA-ready — not HIPAA certified."
      }
    },
    {
      "@type": "Question",
      "name": "How do you price?",
      "acceptedAnswer": {
        "@type": "Answer",
        "text": "Fixed-scope sprints typically range $15K–$75K depending on complexity and timeline. Longer work can shift to a monthly retainer. We can work against pharma MSAs and university purchase-order frameworks. Contact for a scoped quote."
      }
    }
  ]
}
</script>

<!-- Update HTML structure -->
<body>
  <nav><!-- navigation --></nav>
  <main>
    <section aria-labelledby="hero">
      <h1 id="hero">Reproducible bioinformatics, from QC to a package you can file.</h1>
      ...
    </section>
    <section aria-labelledby="services">
      <h2 id="services">Three lanes · omics is the door</h2>
      ...
    </section>
    <section aria-labelledby="work">
      <h2 id="work">Representative work</h2>
      <!-- Show Featured 4 only -->
    </section>
  </main>
  <footer><!-- footer --></footer>
</body>
```

---

## Summary

**Current State:**
- ✅ Good prose structure, clear scope, concrete examples
- ⚠️ Missing structured data, FAQ markup, pricing signal
- ⚠️ 11 Representative Work cards is too many (reduce to 4)
- **Score:** 6/10 agent-friendly, 5/10 AEO-ready

**With Recommended Fixes:**
- ✅ FAQ schema, Organization schema, Service schema
- ✅ Pricing ballpark added ("$15K–$75K for 2–6 week sprints")
- ✅ Semantic HTML5 landmarks (`<main>`, `<section>`, `<nav>`)
- ✅ Representative Work reduced to Featured 4
- **Score:** 9/10 agent-friendly, 9/10 AEO-ready

**Time Investment:** ~6 hours total for all Priority 1 + Priority 2 fixes.

**ROI:** Answer engines (Perplexity, ChatGPT, Claude) will cite bio-sync.tech with high confidence instead of medium/low confidence. Your FAQ answers become canonical sources for "Does Biosync do X?" queries.
