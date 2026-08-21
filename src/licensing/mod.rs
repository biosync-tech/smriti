//! Feature Gating & Licensing — monetization-ready tier system
//!
//! Provides a clean separation between free (Core) and paid (Pro/Enterprise)
//! features. The license key system is simple and local — no phone-home,
//! no cloud validation (consistent with Smriti's offline-first philosophy).

pub mod features;
pub mod key;

pub use features::{FeatureGate, FeatureTier, SmritiFeature};
pub use key::LicenseKey;
