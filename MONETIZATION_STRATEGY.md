# Smriti Monetization Strategy

> Pricing, licensing, distribution, and subscription enforcement for a self-hosted Rust binary.

---

## Table of Contents

1. [Monetization Model](#monetization-model)
2. [Competitive Pricing Analysis](#competitive-pricing-analysis)
3. [Smriti Pricing Tiers](#smriti-pricing-tiers)
4. [Technical Implementation: License Enforcement](#technical-implementation)
5. [Subscription Lifecycle & Cancellation](#subscription-lifecycle)
6. [Distribution Channels](#distribution-channels)
7. [Revenue Projections](#revenue-projections)
8. [Go-to-Market Playbook](#go-to-market-playbook)

---

## 1. Monetization Model <a name="monetization-model"></a>

### Why Open Core + Optional Managed Service

Smriti's core identity is **self-hosted, single binary, zero cloud**. This rules out pure SaaS, but opens three proven monetization paths used by the most successful infrastructure companies:

| Model | Examples | Fit for Smriti |
|-------|----------|---------------|
| **Open Core** | GitLab, Elastic, Meilisearch | **Primary** — free community edition, paid pro/enterprise features |
| **Optional Cloud** | Qdrant, Weaviate, Supabase | **Secondary** — managed hosting for teams who don't want to self-host |
| **Services + Support** | Red Hat, Canonical | **Tertiary** — enterprise SLAs, dedicated support, custom integrations |

**Recommended: Open Core as primary, with optional managed hosting for mid-market.**

### What Goes in Each Tier

The principle: **The community edition must be genuinely useful.** Never cripple the free tier. Instead, gate features that matter specifically to teams and enterprises.

| Feature | Community (Free) | Pro ($29/mo) | Enterprise (Custom) |
|---------|:-:|:-:|:-:|
| Notes CRUD + wiki-links + tags | ✅ | ✅ | ✅ |
| FTS5 full-text search | ✅ | ✅ | ✅ |
| Knowledge graph (full) | ✅ | ✅ | ✅ |
| MCP server (stdio) | ✅ | ✅ | ✅ |
| CLI (all 13 commands) | ✅ | ✅ | ✅ |
| Web dashboard | ✅ | ✅ | ✅ |
| Import/export | ✅ | ✅ | ✅ |
| Agent memory (basic) | ✅ (1 agent, 1 namespace) | ✅ (unlimited) | ✅ |
| Smart link suggestions | — | ✅ | ✅ |
| Daily digest | — | ✅ | ✅ |
| Semantic search (sqlite-vec) | — | ✅ | ✅ |
| Hybrid search (RRF) | — | ✅ | ✅ |
| Conflict resolution (4 policies) | — | ✅ | ✅ |
| Memory history / audit trail | — | ✅ | ✅ |
| Typed graph layers (filter by type) | — | ✅ | ✅ |
| MCP HTTP transport | — | ✅ | ✅ |
| WebDAV / filesystem sync | — | ✅ | ✅ |
| Multi-agent memory (unlimited agents) | — | ✅ | ✅ |
| Tool execution logging | — | ✅ | ✅ |
| SSO / SAML | — | — | ✅ |
| RBAC (role-based access) | — | — | ✅ |
| Compliance export (SOC 2, HIPAA) | — | — | ✅ |
| Federated sync (multi-instance) | — | — | ✅ |
| Priority support + SLA | — | — | ✅ |
| Custom link type registries | — | — | ✅ |
| On-premise deployment assistance | — | — | ✅ |

---

## 2. Competitive Pricing Analysis <a name="competitive-pricing-analysis"></a>

### Agent Memory & Knowledge Infrastructure

| Product | Free Tier | Paid Starts At | Enterprise | Model |
|---------|-----------|---------------|------------|-------|
| **Mem0** | Self-hosted OSS | ~$0.01-0.02/op (cloud) | Custom | Usage-based cloud |
| **Zep** | Community OSS | ~$20-49/mo (cloud) | Custom | Usage-based cloud |
| **Letta** | OSS (MemGPT) | Managed cloud (nascent) | Custom | Cloud-first |
| **LangSmith** | 5K traces/mo | $39/seat/mo | Custom | Per-seat + usage |
| **Pinecone** | Limited free | ~$0.33/1M reads | Custom | Usage-based |
| **Weaviate** | 14-day sandbox | ~$0.095/1M dims | From $1,380/mo | Usage/dedicated |
| **Qdrant** | 1GB free (cloud) | ~$0.045/hr | Custom | Usage-based |

### Knowledge Management (Consumer/Prosumer)

| Product | Free Tier | Paid | Enterprise | Model |
|---------|-----------|------|------------|-------|
| **Obsidian** | Personal use free | $50/user/yr (commercial) | N/A | License + optional services |
| **Obsidian Sync** | — | $4-5/mo | — | Add-on service |
| **Obsidian Publish** | — | $8-10/mo | — | Add-on service |
| **Notion** | Personal free | $8-10/seat/mo | $15-18/seat/mo | Per-seat |
| **Raycast** | Free core | $8/mo (Pro) | $12/user/mo | Per-seat |

### Developer Tool Infrastructure (Self-Hosted)

| Product | Free | Paid | Enforcement | Notes |
|---------|------|------|-------------|-------|
| **Teleport** | <100 employees | Custom | X.509 certificate | 30-day grace period |
| **CockroachDB** | <$10M revenue | CPU-based | BSL license | Mandatory telemetry on free |
| **GitLab** | Community Edition | $29/user/mo (Premium) | Feature flags | $99/user/mo Ultimate |
| **Meilisearch** | OSS | Cloud pricing | Runtime key | API-key-gated cloud features |
| **HashiCorp Vault** | OSS | ~$70/resource/mo | BSL license | Enterprise binary |

### Pricing Insights

1. **$25-39/mo is the sweet spot** for individual developer tools (LangSmith, GitLab, Raycast)
2. **$50/user/year** works for local-first tools (Obsidian commercial)
3. **Enterprise starts at $100+/seat/mo** or custom pricing
4. **Usage-based works for infrastructure** (Pinecone, Weaviate) but adds complexity
5. **The Obsidian model is the closest analog** — local-first app, free for personal, paid for commercial + optional cloud services

---

## 3. Smriti Pricing Tiers <a name="smriti-pricing-tiers"></a>

### Tier 1: Community (Free, Forever)

**Target:** Individual developers, hobbyists, evaluation.

```
Price: $0
License: MIT (source available on GitHub)
Distribution: crates.io, GitHub Releases, Homebrew, Docker Hub
```

**Includes:**
- Full notes CRUD with wiki-links and tags
- FTS5 full-text search
- Complete knowledge graph (petgraph)
- MCP server (stdio transport)
- All 13 CLI commands
- Web dashboard
- Import/export (markdown)
- Agent memory: 1 agent, 1 namespace, no TTL, overwrite-only
- Up to 10,000 notes
- Community support (GitHub Issues/Discussions)

**Rationale:** Must be genuinely useful. Developers who love the free tier become champions who bring Smriti into their organizations.

---

### Tier 2: Pro ($29/month or $290/year)

**Target:** Professional developers, AI engineers, small teams (1-5 people).

```
Price: $29/mo (monthly) or $24.17/mo (annual, save 17%)
License: Runtime license key (Ed25519 signed)
Distribution: Same binary, unlocked by license key
```

**Everything in Community, plus:**
- Semantic search (sqlite-vec KNN)
- Hybrid search (FTS5 + vector, reciprocal rank fusion)
- Smart link suggestions (automatic relationship discovery)
- Daily digest (activity summary, trending topics, orphans)
- All 4 conflict resolution policies (overwrite, reject, version_and_keep, invalidate)
- Memory history / audit trail
- Typed graph layers with filtered traversal
- Multi-agent memory (unlimited agents, namespaces, TTL)
- Tool execution logging
- WebDAV + filesystem sync
- MCP HTTP transport
- Unlimited notes
- Email support (48-hour response)

**Rationale:** $29/mo is the proven price point for individual developer tools. Annual discount drives retention. Every pro feature has clear value for AI agent workflows.

---

### Tier 3: Team ($19/user/month, minimum 5 users)

**Target:** Engineering teams, AI/ML teams, research groups.

```
Price: $19/user/mo (annual only, billed annually)
Minimum: 5 users ($95/mo minimum)
License: Team license key (Ed25519 signed, user count embedded)
```

**Everything in Pro, plus:**
- Shared knowledge graph across team
- Team namespaces for agent memory
- User-scoped API keys
- Usage analytics dashboard
- Onboarding session (30 min)
- Slack/Discord support channel
- Priority email support (24-hour response)

**Rationale:** $19/user/mo undercuts GitLab ($29), Notion ($15-18), while being specifically valuable for AI teams. Annual-only simplifies billing and improves retention.

---

### Tier 4: Enterprise (Custom Pricing)

**Target:** Companies with compliance requirements, 50+ users, regulated industries.

```
Price: Custom (starting at $10,000/year)
License: Enterprise license with custom terms
Distribution: Dedicated binary + deployment support
```

**Everything in Team, plus:**
- SSO / SAML 2.0 integration
- Role-based access control (RBAC)
- Compliance export (SOC 2, HIPAA audit log format)
- Federated sync (multi-instance knowledge graphs)
- Custom link type registries with validation rules
- Dedicated support engineer
- SLA: 4-hour response, 99.9% uptime guarantee (for managed option)
- On-premise deployment assistance
- Custom integrations and consulting
- Volume discounts (100+ users)

**Rationale:** Enterprise features (SSO, RBAC, compliance) are table stakes for procurement. Custom pricing allows value-based negotiation. $10K/yr floor ensures qualified leads only.

---

### Tier 5: Managed Cloud (Future — Q4 2026)

**Target:** Teams who want Smriti without managing infrastructure.

```
Price: $49/mo (Starter, 1 instance) | $199/mo (Growth, 3 instances) | Custom (Enterprise)
```

**Why later:** Building a managed service too early splits focus. Launch only after self-hosted has strong PMF (>1,000 active Pro users).

---

### Price Comparison Summary

| | Smriti | Mem0 Cloud | Zep Cloud | LangSmith | Obsidian |
|---|---|---|---|---|---|
| **Individual** | $29/mo | Usage-based | $20-49/mo | $39/seat/mo | $50/yr |
| **Team (10 users)** | $190/mo | Custom | Custom | $390/mo | $500/yr |
| **Enterprise** | From $10K/yr | Custom | Custom | Custom | N/A |
| **Self-hosted free** | Yes | Yes | Yes | No | Yes |

---

## 4. Technical Implementation: License Enforcement <a name="technical-implementation"></a>

### Architecture

```
┌────────────────────────────────────────────┐
│                Smriti Binary                │
│                                            │
│  ┌─────────────────────────────────────┐   │
│  │         License Validator            │   │
│  │  • Embedded Ed25519 public key       │   │
│  │  • Validates signed license JSON     │   │
│  │  • Caches validation for 7 days      │   │
│  │  • Checks online weekly              │   │
│  └─────────────┬───────────────────────┘   │
│                │                            │
│  ┌─────────────▼───────────────────────┐   │
│  │         Feature Gate                 │   │
│  │  • Community: always allowed         │   │
│  │  • Pro: requires valid license       │   │
│  │  • Enterprise: requires ent license  │   │
│  └─────────────────────────────────────┘   │
└────────────────────────────────────────────┘
         ▲ online validation (weekly)
         │
┌────────┴───────────────────────────────────┐
│          License Server (Polar.sh)          │
│  • Issues license keys on payment           │
│  • Revokes on subscription cancellation     │
│  • Webhook → Stripe/payment provider        │
└─────────────────────────────────────────────┘
```

### New Dependencies

```toml
# Add to Cargo.toml
ed25519-dalek = { version = "2", features = ["serde"] }
```

No other new dependencies needed — `serde_json`, `chrono`, `reqwest`, `base64`, and `sha2` are already in `Cargo.toml`.

### Core Implementation

```rust
// src/license/mod.rs

use chrono::{DateTime, Utc, Duration};
use ed25519_dalek::{VerifyingKey, Signature, Verifier};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Embedded at compile time — safe to ship in the binary.
const PUBLIC_KEY_BYTES: &[u8; 32] = include_bytes!("../../keys/license.pub");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Community,
    Pro,
    Team,
    Enterprise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicensePayload {
    pub license_id: String,
    pub customer_id: String,
    pub customer_email: String,
    pub tier: Tier,
    pub max_users: Option<u32>,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedLicense {
    pub payload: String,      // JSON string of LicensePayload
    pub signature: String,    // base64-encoded Ed25519 signature
}

#[derive(Debug, Clone)]
pub struct LicenseState {
    pub tier: Tier,
    pub features: Vec<String>,
    pub expires_at: DateTime<Utc>,
    pub grace_deadline: DateTime<Utc>,  // expires_at + 14 days
    pub valid: bool,
    pub degraded: bool,                 // in grace period
}

impl LicenseState {
    /// Community tier — no license needed.
    pub fn community() -> Self {
        Self {
            tier: Tier::Community,
            features: vec![],
            expires_at: Utc::now() + Duration::days(36500), // effectively never
            grace_deadline: Utc::now() + Duration::days(36500),
            valid: true,
            degraded: false,
        }
    }

    /// Check if a specific feature is available.
    pub fn check_feature(&self, feature: &str) -> Result<(), crate::errors::AppError> {
        // Community features are always available
        if is_community_feature(feature) {
            return Ok(());
        }

        let now = Utc::now();

        if now > self.grace_deadline {
            return Err(crate::errors::AppError::BadRequest(
                format!("License expired. Feature '{}' requires an active {} license.",
                        feature, tier_name(&self.tier))
            ));
        }

        if !self.features.contains(&feature.to_string()) {
            return Err(crate::errors::AppError::BadRequest(
                format!("Feature '{}' is not included in your {} plan. Upgrade at https://smritiai.netlify.app/pricing",
                        feature, tier_name(&self.tier))
            ));
        }

        Ok(())
    }

    pub fn is_active(&self) -> bool {
        Utc::now() < self.grace_deadline
    }
}

fn tier_name(tier: &Tier) -> &'static str {
    match tier {
        Tier::Community => "Community",
        Tier::Pro => "Pro",
        Tier::Team => "Team",
        Tier::Enterprise => "Enterprise",
    }
}

fn is_community_feature(feature: &str) -> bool {
    matches!(feature,
        "notes_crud" | "fts_search" | "knowledge_graph" |
        "mcp_stdio" | "cli" | "web_dashboard" | "import_export" |
        "basic_memory"
    )
}

/// Validate a signed license against the embedded public key.
pub fn validate_license(signed: &SignedLicense) -> Result<LicensePayload, crate::errors::AppError> {
    let public_key = VerifyingKey::from_bytes(PUBLIC_KEY_BYTES)
        .map_err(|_| crate::errors::AppError::BadRequest("Invalid license key".into()))?;

    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(&signed.signature)
        .map_err(|_| crate::errors::AppError::BadRequest("Invalid license signature".into()))?;

    let signature = Signature::from_slice(&sig_bytes)
        .map_err(|_| crate::errors::AppError::BadRequest("Invalid license signature".into()))?;

    public_key
        .verify(signed.payload.as_bytes(), &signature)
        .map_err(|_| crate::errors::AppError::BadRequest("License signature verification failed".into()))?;

    let payload: LicensePayload = serde_json::from_str(&signed.payload)
        .map_err(|_| crate::errors::AppError::BadRequest("Invalid license payload".into()))?;

    if payload.expires_at < Utc::now() {
        // Still return the payload — caller handles grace period
    }

    Ok(payload)
}

/// Load license from file or environment variable.
pub fn load_license() -> LicenseState {
    // Priority: SMRITI_LICENSE env var > ~/.config/smriti/license.json > community
    let license_json = std::env::var("SMRITI_LICENSE").ok()
        .or_else(|| {
            let path = dirs::config_dir()?.join("smriti").join("license.json");
            std::fs::read_to_string(path).ok()
        });

    let Some(json) = license_json else {
        return LicenseState::community();
    };

    let Ok(signed) = serde_json::from_str::<SignedLicense>(&json) else {
        eprintln!("[smriti] Warning: Invalid license file format. Running as Community.");
        return LicenseState::community();
    };

    match validate_license(&signed) {
        Ok(payload) => {
            let now = Utc::now();
            let grace_deadline = payload.expires_at + Duration::days(14);
            let valid = payload.expires_at > now;
            let degraded = !valid && grace_deadline > now;

            if degraded {
                eprintln!("[smriti] Warning: License expired. Grace period ends {}.",
                         grace_deadline.format("%Y-%m-%d"));
            }

            LicenseState {
                tier: payload.tier,
                features: payload.features,
                expires_at: payload.expires_at,
                grace_deadline,
                valid,
                degraded,
            }
        }
        Err(e) => {
            eprintln!("[smriti] License validation failed: {}. Running as Community.", e);
            LicenseState::community()
        }
    }
}
```

### Feature Gating at API Boundaries

```rust
// In MCP handler (src/mcp/handlers.rs)
pub fn handle_notes_search_semantic(&self, params: &Value) -> Result<Value, (i32, String)> {
    self.license.check_feature("semantic_search")
        .map_err(|e| (-32001, e.to_string()))?;
    // ... actual implementation
}

// In REST API handler (src/api/routes/semantic.rs)
pub async fn hybrid_search(
    State(state): State<AppState>,
    Json(query): Json<HybridSearchQuery>,
) -> Result<Json<Vec<HybridSearchResult>>, AppError> {
    state.license.check_feature("hybrid_search")?;
    // ... actual implementation
}

// In CLI handler (src/cli/handlers.rs)
pub fn handle_stats(db: &Database, license: &LicenseState) -> AppResult<()> {
    // Basic stats: always available
    print_basic_stats(db)?;

    // Smart link suggestions: pro feature
    if license.check_feature("smart_links").is_ok() {
        print_smart_link_suggestions(db)?;
    } else {
        println!("  Smart link suggestions available in Smriti Pro.");
        println!("  Upgrade: https://smritiai.netlify.app/pricing");
    }
    Ok(())
}
```

### Feature Map by Tier

```rust
// Features included in each tier
pub fn pro_features() -> Vec<String> {
    vec![
        "semantic_search", "hybrid_search", "smart_links",
        "daily_digest", "conflict_resolution", "memory_history",
        "typed_graph_layers", "multi_agent", "tool_logging",
        "webdav_sync", "mcp_http", "unlimited_notes",
    ].into_iter().map(String::from).collect()
}

pub fn team_features() -> Vec<String> {
    let mut f = pro_features();
    f.extend(vec![
        "shared_graph", "team_namespaces", "user_api_keys",
        "usage_analytics",
    ].into_iter().map(String::from));
    f
}

pub fn enterprise_features() -> Vec<String> {
    let mut f = team_features();
    f.extend(vec![
        "sso_saml", "rbac", "compliance_export",
        "federated_sync", "custom_link_types",
    ].into_iter().map(String::from));
    f
}
```

---

## 5. Subscription Lifecycle & Cancellation <a name="subscription-lifecycle"></a>

### Payment → License → Revocation Flow

```
Customer Journey:

1. PURCHASE                2. ACTIVATE               3. USE
   smritiai.netlify.app/pricing  →   smriti activate <key>  →  All pro features
   Polar.sh checkout       Writes license.json       unlocked
   Stripe payment          to ~/.config/smriti/

4. RENEWAL (auto)          5. CANCELLATION           6. GRACE PERIOD
   Polar.sh charges  →     Customer cancels     →    14-day grace
   License extended         Polar.sh webhook          Pro features
   No action needed         marks license expired     still work

7. EXPIRATION              8. DEGRADATION
   Grace period ends  →    Binary continues running
   Pro features return     as Community edition
   friendly error msgs     No data loss, no crashes
```

### Key Design Decisions

**1. No phone-home requirement for daily use.**
The license is validated cryptographically offline. Online validation happens:
- On first activation (`smriti activate <key>`)
- Weekly background check (if internet available)
- On explicit `smriti license check` command

**2. Graceful degradation, never bricking.**
When a license expires:
- All existing data remains accessible
- Community features continue working
- Pro features return clear, friendly error messages with upgrade links
- No data deletion, no database corruption, no locked files

**3. 14-day grace period after cancellation.**
Industry standard. Prevents accidental lockouts from payment failures.

**4. License file is portable.**
`~/.config/smriti/license.json` can be copied to any machine. Team licenses embed a user count; the binary doesn't need to phone home to verify user count (honor system for self-hosted, auditable for enterprise).

### CLI Commands for License Management

```bash
# Activate a new license
smriti activate SMRITI-PRO-XXXX-XXXX-XXXX-XXXX

# Check license status
smriti license
# Output:
#   License: SMRITI-PRO-XXXX (active)
#   Tier: Pro
#   Expires: 2027-04-01
#   Features: 12 enabled
#   Status: ✅ Valid

# After cancellation:
smriti license
# Output:
#   License: SMRITI-PRO-XXXX (grace period)
#   Tier: Pro → Community on 2026-04-15
#   Features: 12 enabled (14 days remaining)
#   Status: ⚠️  Expiring — renew at https://smritiai.netlify.app/pricing

# After grace period:
smriti license
# Output:
#   License: expired
#   Tier: Community
#   Features: 7 (community)
#   Status: Upgrade at https://smritiai.netlify.app/pricing

# Deactivate (remove license)
smriti deactivate
```

---

## 6. Distribution Channels <a name="distribution-channels"></a>

### Multi-Channel Distribution Strategy

```
                     ┌──────────────┐
                     │  Source Code  │
                     │   (GitHub)   │
                     └──────┬───────┘
                            │
              ┌─────────────┼─────────────┐
              ▼             ▼             ▼
        ┌──────────┐  ┌──────────┐  ┌──────────┐
        │ crates.io │  │  GitHub  │  │  Docker  │
        │  (source) │  │ Releases │  │   Hub    │
        │           │  │ (binary) │  │  (image) │
        └─────┬────┘  └────┬─────┘  └────┬─────┘
              │             │             │
              ▼             ▼             ▼
        cargo install   Direct DL    docker pull
                        Homebrew     docker compose
```

| Channel | Command | Audience | License |
|---------|---------|----------|---------|
| **crates.io** | `cargo install smriti` | Rust developers | Community (source) |
| **Homebrew** | `brew install biosync-tech/tap/smriti` | macOS developers | Community binary |
| **GitHub Releases** | Direct download | All platforms | Community + Pro binary |
| **Docker Hub** | `docker pull smriti/smriti` | Self-hosted servers | Community + Pro |
| **smritiai.netlify.app** | Web download + checkout | Pro/Enterprise buyers | Pro/Enterprise |

### Homebrew Tap Setup

```ruby
# homebrew-smriti/Formula/smriti.rb
class Smriti < Formula
  desc "Self-hosted knowledge graph & agent memory layer"
  homepage "https://smritiai.netlify.app"
  version "0.2.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/biosync-tech/smriti/releases/download/v0.2.0/smriti-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER"
    else
      url "https://github.com/biosync-tech/smriti/releases/download/v0.2.0/smriti-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER"
    end
  end

  on_linux do
    url "https://github.com/biosync-tech/smriti/releases/download/v0.2.0/smriti-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "PLACEHOLDER"
  end

  def install
    bin.install "smriti"
    # Shell completions
    generate_completions_from_executable(bin/"smriti", "completions")
  end

  test do
    assert_match "smriti #{version}", shell_output("#{bin}/smriti --version")
  end
end
```

### GitHub Actions Release Workflow

```yaml
# .github/workflows/release.yml
name: Release
on:
  push:
    tags: ["v*"]

jobs:
  build:
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
          - target: aarch64-unknown-linux-gnu
            os: ubuntu-latest
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-pc-windows-msvc
            os: windows-latest

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - run: cargo build --release --target ${{ matrix.target }}
      - name: Package
        run: |
          tar czf smriti-${{ matrix.target }}.tar.gz \
            -C target/${{ matrix.target }}/release smriti
      - uses: softprops/action-gh-release@v2
        with:
          files: smriti-${{ matrix.target }}.tar.gz

  docker:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: docker/login-action@v3
        with:
          username: ${{ secrets.DOCKERHUB_USERNAME }}
          password: ${{ secrets.DOCKERHUB_TOKEN }}
      - uses: docker/build-push-action@v5
        with:
          push: true
          tags: smriti/smriti:latest,smriti/smriti:${{ github.ref_name }}

  homebrew:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - name: Update Homebrew formula
        run: |
          # Update SHA256 hashes and version in tap repo
          gh workflow dispatch update-formula \
            --repo biosync-tech/homebrew-smriti \
            --field version=${{ github.ref_name }}
```

---

## 7. Revenue Projections <a name="revenue-projections"></a>

### Conservative Model (Year 1-3)

**Assumptions:**
- GitHub stars → users: 2% conversion (industry standard for dev tools)
- Free → Pro conversion: 3-5% (Obsidian benchmarks ~4%)
- Pro → Team upsell: 10% of Pro users bring teams
- Churn: 5% monthly (Pro), 2% monthly (Team/Enterprise)

| Metric | Year 1 | Year 2 | Year 3 |
|--------|--------|--------|--------|
| GitHub stars | 2,000 | 8,000 | 20,000 |
| Active users (free) | 500 | 2,000 | 5,000 |
| Pro subscribers | 20 | 100 | 300 |
| Team customers (avg 8 users) | 2 | 10 | 30 |
| Enterprise customers | 0 | 2 | 8 |
| **Pro MRR** | $580 | $2,900 | $8,700 |
| **Team MRR** | $304 | $1,520 | $4,560 |
| **Enterprise MRR** | $0 | $1,667 | $6,667 |
| **Total MRR** | **$884** | **$6,087** | **$19,927** |
| **Total ARR** | **$10,608** | **$73,044** | **$239,124** |

### Optimistic Model (Strong PMF + Viral Growth)

If Smriti captures the "Obsidian for AI agents" positioning and gets featured in major AI/developer newsletters:

| Metric | Year 1 | Year 2 | Year 3 |
|--------|--------|--------|--------|
| GitHub stars | 5,000 | 25,000 | 60,000 |
| Pro subscribers | 50 | 500 | 2,000 |
| Team customers | 5 | 30 | 100 |
| Enterprise customers | 1 | 8 | 25 |
| **Total ARR** | **$30K** | **$310K** | **$1.2M** |

### Key Revenue Levers

1. **Pro conversion rate:** Every 1% improvement in free→Pro = ~$100K ARR at scale
2. **Team upsell:** Average team deal ($1,824/yr) is 5x a Pro subscription
3. **Enterprise:** Single enterprise deal ($10K+/yr) = 29 Pro subscriptions
4. **Managed cloud (Year 2+):** Higher margins, recurring, less support burden

---

## 8. Go-to-Market Playbook <a name="go-to-market-playbook"></a>

### Phase 1: Developer Adoption (Month 1-6)

**Goal:** 2,000 GitHub stars, 500 active users, 20 Pro subscribers.

| Channel | Action | Cost |
|---------|--------|------|
| **MCP Registry** | List on official MCP registry (modelcontextprotocol.io) | Free |
| **Hacker News** | Launch post: "Show HN: Self-hosted knowledge graph for AI agents" | Free |
| **Reddit** | r/rust, r/selfhosted, r/LocalLLaMA, r/ChatGPTCoding | Free |
| **Dev.to / Hashnode** | Technical posts: "Building agent memory with Smriti" | Free |
| **X/Twitter** | Thread: "Why your AI agent forgets everything (and how to fix it)" | Free |
| **YouTube** | Demo video: 5-minute setup, MCP integration, graph viz | Free |
| **Discord** | Launch community server | Free |
| **awesome-mcp** | PR to awesome-mcp-servers list | Free |

**Key message:** *"Your AI agent has amnesia. Smriti gives it a brain."*

### Phase 2: Pro Conversion (Month 3-12)

**Goal:** 5% free→Pro conversion rate.

| Tactic | Details |
|--------|---------|
| **In-product upgrade prompts** | When user hits a gated feature, show clear upgrade CTA with feature preview |
| **14-day Pro trial** | Auto-activate Pro for new users. After 14 days, gracefully degrade to Community |
| **Comparison page** | smritiai.netlify.app/pricing with feature matrix and competitor comparison |
| **Case studies** | Publish 3-5 user stories (healthcare, research, enterprise) |
| **Newsletter** | Monthly "Smriti Digest" with tips, new features, community highlights |

### Phase 3: Team & Enterprise (Month 6-18)

**Goal:** 10 team customers, 2 enterprise customers.

| Tactic | Details |
|--------|---------|
| **Sales-assisted** | Inbound form on website, 30-min demo calls |
| **Compliance docs** | SOC 2 readiness checklist, HIPAA deployment guide |
| **Integration guides** | Claude Code, Cursor, Windsurf, custom agent frameworks |
| **Partner program** | AI consultancies get 20% commission on referrals |
| **Conference talks** | AI Engineer Summit, RustConf, local meetups |

### Phase 4: Managed Cloud (Month 12+)

**Goal:** Launch smritiai.netlify.app/cloud for teams who don't want to self-host.

| Feature | Differentiator |
|---------|---------------|
| **One-click deploy** | Provision a Smriti instance in 30 seconds |
| **Automatic backups** | Daily SQLite backups, 30-day retention |
| **Team management** | Web-based user/role management |
| **Usage dashboard** | Notes, searches, memory operations, graph size |
| **Custom domains** | `knowledge.yourcompany.com` |

---

## Payment Provider Recommendation

### Primary: Polar.sh

**Why Polar.sh over alternatives:**

| Factor | Polar.sh | Lemon Squeezy | Stripe Direct |
|--------|----------|---------------|---------------|
| Open-source focus | Built for OSS | General digital products | General payments |
| License key management | Built-in, auto-revoke | Built-in, manual | Must build yourself |
| Merchant of Record | Yes (handles tax) | Yes | No (you handle tax) |
| GitHub integration | Native | None | None |
| Pricing | 5% fee | 5% + $0.50/txn | 2.9% + $0.30/txn |
| Setup complexity | Low | Low | High |
| License auto-revocation | On cancellation | On expiry | Must build with webhooks |

**Implementation:**
1. Create product tiers on Polar.sh
2. Configure license key generation (branded: `SMRITI-PRO-XXXX`)
3. Embed checkout link on smritiai.netlify.app/pricing
4. In the binary, validate against Polar.sh API (or offline with Ed25519)
5. Polar.sh handles tax, invoicing, refunds, and revocation automatically

### Fallback: Stripe + Keygen.sh

For enterprise customers who need custom invoicing, PO-based purchasing, or multi-year contracts, use Stripe directly with Keygen.sh for license management. Keygen.sh is self-hostable (Fair Core License) if you need to keep the license server on-premise too.

---

## Implementation Priority

| Priority | Task | Effort | Revenue Impact |
|----------|------|--------|---------------|
| 1 | Add `LicenseState` to `AppState`, gate 12 pro features | 2 days | Enables Pro tier |
| 2 | `smriti activate` / `smriti license` CLI commands | 1 day | User experience |
| 3 | Polar.sh product setup + checkout page | 1 day | Accepts payments |
| 4 | smritiai.netlify.app/pricing page | 2 days | Conversion funnel |
| 5 | GitHub Actions release workflow (multi-platform) | 1 day | Distribution |
| 6 | Homebrew tap | 0.5 days | macOS distribution |
| 7 | 14-day trial auto-activation | 0.5 days | Conversion rate |
| 8 | In-product upgrade prompts | 1 day | Conversion rate |
| **Total** | | **~9 days** | |

---

## Summary for Decision Makers

1. **Community edition stays free forever.** This is non-negotiable for developer trust and adoption.
2. **Pro at $29/mo** captures individual AI engineers. Low friction, self-serve, instant activation.
3. **Team at $19/user/mo** captures engineering teams. Volume discount drives adoption.
4. **Enterprise is custom** because every enterprise deal is different.
5. **License enforcement is cryptographic, offline-first, and graceful.** No phoning home for daily use. No bricking. No data hostage.
6. **Distribution is multi-channel** from day one: crates.io, Homebrew, Docker, GitHub Releases.
7. **Polar.sh handles payments, tax, licensing, and revocation.** One integration, global compliance.
8. **9 engineering days** from zero to accepting payments.

---

*Document generated April 2026. Pricing benchmarked against Mem0, Zep, Letta, LangSmith, Obsidian, Raycast, and enterprise infrastructure tools.*
