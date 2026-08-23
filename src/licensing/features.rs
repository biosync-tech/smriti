//! Feature definitions and tier assignments

use serde::{Deserialize, Serialize};

/// Feature tiers for monetization
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureTier {
    /// Free tier — core knowledge graph features
    Core,
    /// Paid tier — AI-powered features
    Pro,
    /// Premium tier — multi-user, advanced analytics
    Enterprise,
}

/// Named features that can be gated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmritiFeature {
    // Core (free)
    NoteCrud,
    FullTextSearch,
    WikiLinks,
    Tags,
    KnowledgeGraph,
    ManualEmbeddings,
    Cli,
    RestApi,
    McpServer,
    SyncEngine,

    // Pro (paid)
    LocalInference,
    AutoEmbedding,
    RagQuery,
    AiSmartLinking,
    AiAutoTagging,
    AiSummarization,
    AiDailyDigest,
    MultimodalIngest,
    WebDashboard,

    // Enterprise
    MultiUser,
    Rbac,
    AuditLog,
    CustomModels,
    PrioritySupport,
}

impl SmritiFeature {
    /// Get the minimum tier required for this feature
    pub fn required_tier(&self) -> FeatureTier {
        match self {
            // Core features
            SmritiFeature::NoteCrud
            | SmritiFeature::FullTextSearch
            | SmritiFeature::WikiLinks
            | SmritiFeature::Tags
            | SmritiFeature::KnowledgeGraph
            | SmritiFeature::ManualEmbeddings
            | SmritiFeature::Cli
            | SmritiFeature::RestApi
            | SmritiFeature::McpServer
            | SmritiFeature::SyncEngine => FeatureTier::Core,

            // Pro features
            SmritiFeature::LocalInference
            | SmritiFeature::AutoEmbedding
            | SmritiFeature::RagQuery
            | SmritiFeature::AiSmartLinking
            | SmritiFeature::AiAutoTagging
            | SmritiFeature::AiSummarization
            | SmritiFeature::AiDailyDigest
            | SmritiFeature::MultimodalIngest
            | SmritiFeature::WebDashboard => FeatureTier::Pro,

            // Enterprise features
            SmritiFeature::MultiUser
            | SmritiFeature::Rbac
            | SmritiFeature::AuditLog
            | SmritiFeature::CustomModels
            | SmritiFeature::PrioritySupport => FeatureTier::Enterprise,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            SmritiFeature::NoteCrud => "Note CRUD",
            SmritiFeature::FullTextSearch => "Full-Text Search",
            SmritiFeature::WikiLinks => "Wiki Links",
            SmritiFeature::Tags => "Tags",
            SmritiFeature::KnowledgeGraph => "Knowledge Graph",
            SmritiFeature::ManualEmbeddings => "Manual Embeddings",
            SmritiFeature::Cli => "CLI",
            SmritiFeature::RestApi => "REST API",
            SmritiFeature::McpServer => "MCP Server",
            SmritiFeature::SyncEngine => "Sync Engine",
            SmritiFeature::LocalInference => "Local AI Inference",
            SmritiFeature::AutoEmbedding => "Auto-Embedding",
            SmritiFeature::RagQuery => "RAG Query",
            SmritiFeature::AiSmartLinking => "AI Smart Linking",
            SmritiFeature::AiAutoTagging => "AI Auto-Tagging",
            SmritiFeature::AiSummarization => "AI Summarization",
            SmritiFeature::AiDailyDigest => "AI Daily Digest",
            SmritiFeature::MultimodalIngest => "Multimodal Ingestion",
            SmritiFeature::WebDashboard => "Web Dashboard",
            SmritiFeature::MultiUser => "Multi-User",
            SmritiFeature::Rbac => "Role-Based Access Control",
            SmritiFeature::AuditLog => "Audit Logging",
            SmritiFeature::CustomModels => "Custom Models",
            SmritiFeature::PrioritySupport => "Priority Support",
        }
    }
}

/// Feature gate that checks if a feature is available
#[derive(Debug, Clone)]
pub struct FeatureGate {
    active_tier: FeatureTier,
}

impl FeatureGate {
    pub fn new(tier: FeatureTier) -> Self {
        Self { active_tier: tier }
    }

    /// Create a gate that allows all features (for development)
    pub fn unrestricted() -> Self {
        Self {
            active_tier: FeatureTier::Enterprise,
        }
    }

    /// Create a gate for the free tier
    pub fn free() -> Self {
        Self {
            active_tier: FeatureTier::Core,
        }
    }

    /// Check if a feature is available
    pub fn is_available(&self, feature: SmritiFeature) -> bool {
        self.active_tier >= feature.required_tier()
    }

    /// Check and return error if feature is not available
    pub fn require(&self, feature: SmritiFeature) -> Result<(), String> {
        if self.is_available(feature) {
            Ok(())
        } else {
            Err(format!(
                "Feature '{}' requires {} tier. Current tier: {:?}. \
                 Upgrade at https://smriti.dev/pricing",
                feature.display_name(),
                match feature.required_tier() {
                    FeatureTier::Core => "Core",
                    FeatureTier::Pro => "Pro",
                    FeatureTier::Enterprise => "Enterprise",
                },
                self.active_tier,
            ))
        }
    }

    pub fn active_tier(&self) -> FeatureTier {
        self.active_tier
    }

    /// List all features available at the current tier
    pub fn available_features(&self) -> Vec<SmritiFeature> {
        use SmritiFeature::*;
        let all = vec![
            NoteCrud,
            FullTextSearch,
            WikiLinks,
            Tags,
            KnowledgeGraph,
            ManualEmbeddings,
            Cli,
            RestApi,
            McpServer,
            SyncEngine,
            LocalInference,
            AutoEmbedding,
            RagQuery,
            AiSmartLinking,
            AiAutoTagging,
            AiSummarization,
            AiDailyDigest,
            MultimodalIngest,
            WebDashboard,
            MultiUser,
            Rbac,
            AuditLog,
            CustomModels,
            PrioritySupport,
        ];
        all.into_iter().filter(|f| self.is_available(*f)).collect()
    }
}

impl Default for FeatureGate {
    fn default() -> Self {
        // Default to unrestricted during development
        // Change to Self::free() before shipping
        Self::unrestricted()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_tier_ordering() {
        assert!(FeatureTier::Core < FeatureTier::Pro);
        assert!(FeatureTier::Pro < FeatureTier::Enterprise);
    }

    #[test]
    fn test_feature_gate_requires() {
        let gate = FeatureGate::new(FeatureTier::Pro);
        assert!(gate.require(SmritiFeature::RagQuery).is_ok());
        assert!(gate.require(SmritiFeature::MultiUser).is_err());
    }

    #[test]
    fn test_feature_gate_unrestricted() {
        let gate = FeatureGate::unrestricted();
        assert!(gate.is_available(SmritiFeature::PrioritySupport));
    }

    #[test]
    fn test_feature_gate_free() {
        let gate = FeatureGate::free();
        assert!(gate.is_available(SmritiFeature::NoteCrud));
        assert!(!gate.is_available(SmritiFeature::RagQuery));
    }
}
