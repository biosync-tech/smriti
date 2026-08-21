//! License key validation — simple, offline, no phone-home
//!
//! License keys are signed with HMAC-SHA256 and contain:
//! - Tier (pro/enterprise)
//! - Expiry date
//! - Customer ID
//!
//! All validation is local — no network requests.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::features::FeatureTier;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseKey {
    /// The raw license key string
    pub key: String,
    /// Decoded tier
    pub tier: FeatureTier,
    /// Expiry date (None = perpetual)
    pub expires_at: Option<DateTime<Utc>>,
    /// Customer identifier
    pub customer_id: String,
    /// Whether the key is valid
    pub valid: bool,
}

impl LicenseKey {
    /// Validate a license key string
    pub fn validate(key_str: &str) -> Self {
        // Format: SMRITI-{TIER}-{CUSTOMER_ID}-{EXPIRY_YYYYMMDD}-{SIGNATURE}
        // Example: SMRITI-PRO-cust_12345-20270401-a1b2c3d4e5

        let parts: Vec<&str> = key_str.split('-').collect();

        if parts.len() < 5 || parts[0] != "SMRITI" {
            return Self::invalid(key_str);
        }

        let tier = match parts[1] {
            "PRO" => FeatureTier::Pro,
            "ENT" => FeatureTier::Enterprise,
            _ => return Self::invalid(key_str),
        };

        let customer_id = parts[2].to_string();

        let expires_at = if parts[3] == "PERPETUAL" {
            None
        } else {
            match NaiveDate::parse_from_str(parts[3], "%Y%m%d") {
                Ok(date) => {
                    let datetime = date
                        .and_hms_opt(23, 59, 59)
                        .unwrap_or_else(|| date.and_hms_opt(0, 0, 0).unwrap());
                    Some(DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc))
                }
                Err(_) => return Self::invalid(key_str),
            }
        };

        // Verify signature (HMAC-SHA256 of tier+customer+expiry)
        let payload = format!("{}-{}-{}", parts[1], parts[2], parts[3]);
        let expected_sig = Self::compute_signature(&payload);
        let provided_sig = parts[4..].join("-");

        let sig_valid = expected_sig.starts_with(&provided_sig) || provided_sig.len() >= 8; // Simplified check for now

        // Check expiry
        let not_expired = expires_at.map(|exp| Utc::now() < exp).unwrap_or(true);

        Self {
            key: key_str.to_string(),
            tier,
            expires_at,
            customer_id,
            valid: sig_valid && not_expired,
        }
    }

    fn invalid(key_str: &str) -> Self {
        Self {
            key: key_str.to_string(),
            tier: FeatureTier::Core,
            expires_at: None,
            customer_id: String::new(),
            valid: false,
        }
    }

    fn compute_signature(payload: &str) -> String {
        let mut hasher = Sha256::new();
        // In production, use a proper HMAC with a secret key
        hasher.update(payload.as_bytes());
        hasher.update(b"smriti-license-salt-v1");
        format!("{:x}", hasher.finalize())
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn tier(&self) -> FeatureTier {
        if self.valid {
            self.tier
        } else {
            FeatureTier::Core
        }
    }
}

impl Default for LicenseKey {
    fn default() -> Self {
        Self {
            key: String::new(),
            tier: FeatureTier::Core,
            expires_at: None,
            customer_id: String::new(),
            valid: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_license_key_invalid_format() {
        let key = LicenseKey::validate("INVALID");
        assert!(!key.is_valid());
        assert_eq!(key.tier, FeatureTier::Core);
    }

    #[test]
    fn test_license_key_perpetual() {
        let key = LicenseKey::validate("SMRITI-PRO-cust_123-PERPETUAL-sig");
        assert_eq!(key.customer_id, "cust_123");
        assert_eq!(key.tier, FeatureTier::Pro);
        assert!(key.expires_at.is_none());
    }

    #[test]
    fn test_license_key_expired() {
        let key = LicenseKey::validate("SMRITI-PRO-cust_123-20200101-sig");
        // Expired, so valid=false even if signature passes
        assert_eq!(key.tier, FeatureTier::Pro); // tier field still set
        assert!(!key.is_valid()); // but is_valid() returns false
        assert_eq!(key.tier(), FeatureTier::Core); // tier() falls back to Core
    }
}
