use std::path::PathBuf;

/// Default store used by CLI and MCP when `--db` / `SMRITI_DB` are unset.
/// Matches the existing Python agent helpers (`~/.smriti/smriti.db`).
pub fn default_db_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".smriti").join("smriti.db")
}

/// Resolve the database path: `--db` > `SMRITI_DB` > `~/.smriti/smriti.db`.
pub fn resolve_db_path(explicit: Option<&str>) -> String {
    if let Some(p) = explicit {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Ok(p) = std::env::var("SMRITI_DB") {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    default_db_path().to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_path_wins() {
        assert_eq!(resolve_db_path(Some("/tmp/smoke.db")), "/tmp/smoke.db");
    }

    #[test]
    fn empty_explicit_falls_through() {
        let got = resolve_db_path(Some("  "));
        assert!(got.ends_with("smriti.db"), "{got}");
    }
}
