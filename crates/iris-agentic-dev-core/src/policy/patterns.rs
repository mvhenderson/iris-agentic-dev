//! Hardcoded PHI name patterns and system global blocklist (051-phi-policy-env-gates).
//!
//! Sources: Pierre Abdelsayed's servermanager-3.13.0-build.0D05.vsix `mcp-server.js`
//! arrays `N` (PHI patterns) and `D` (system blocklist).

/// Hardcoded system global blocklist. Non-configurable, enforced regardless of `dataPolicy`.
/// Per-connection `globalBlocklist` entries EXTEND (not replace) this list.
pub const SYSTEM_BLOCKLIST: &[&str] = &[
    "^%SYS*",
    "^%Library*",
    "^%Dictionary*",
    "^%SYSTEM*",
    "^rOBJ",
    "^rMAP",
    "^rINDEX",
    "^rINCLUDE",
    "^rBACKUP",
    "^ROUTINE",
    "^oddDEF",
    "^oddEXT",
    "^oddSQL",
    "^oddMAC",
    "^oddPKG",
    "^oddCOM",
    "^ROLE",
    "^USER",
    "^Ens.Config*",
    "^Ens.Rule*",
    "^Ens.Rules*",
    "^Ens.MessageHeader*",
    "^Ens.MessageBody*",
    "^SYS*",
    "^SYSTEM*",
    "^DeepSee*",
    "^IRIS.Msg*",
    "^IRIS.Temp*",
    "^IRIS.Sys*",
    "^IRIS.SysLog*",
];

/// Hardcoded PHI name patterns. Globals matching these require `acknowledgePhi: true`
/// for individual reads. Does NOT apply to bulk-PHI tools (`journal_search`, `view_message_body`).
pub const PHI_NAME_PATTERNS: &[&str] = &[
    "^PAPMI*",
    "^PAADM*",
    "^PAAPT*",
    "^PAPER*",
    "^MRADM*",
    "^OE*",
    "^ORDER*",
    "^Ens.MessageHeader*",
    "^Ens.MessageBody*",
];

/// Returns `true` if `global_name` matches `pattern`.
///
/// Pattern rules:
/// - Leading `^` is stripped (IRIS global naming convention, not a regex anchor).
/// - If pattern ends with `*`: prefix match against the stripped pattern.
/// - Otherwise: exact match.
/// - Matching is case-insensitive.
pub fn matches_pattern(global_name: &str, pattern: &str) -> bool {
    let p = pattern.strip_prefix('^').unwrap_or(pattern);
    let name_upper = global_name.to_uppercase();
    if let Some(prefix) = p.strip_suffix('*') {
        name_upper.starts_with(&prefix.to_uppercase())
    } else {
        name_upper == p.to_uppercase()
    }
}

/// Returns `true` if `global_name` matches any pattern in `patterns`.
pub fn matches_any(global_name: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| matches_pattern(global_name, p))
}

/// Returns `true` if `global_name` matches any pattern in a `Vec<String>`.
pub fn matches_any_owned(global_name: &str, patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|p| matches_pattern(global_name, p.as_str()))
}

/// Returns the first matching pattern from `patterns`, or `None`.
pub fn first_match<'a>(global_name: &str, patterns: &[&'a str]) -> Option<&'a str> {
    patterns
        .iter()
        .copied()
        .find(|p| matches_pattern(global_name, p))
}

/// Returns the first matching pattern from an owned slice, cloned.
pub fn first_match_owned(global_name: &str, patterns: &[String]) -> Option<String> {
    patterns
        .iter()
        .find(|p| matches_pattern(global_name, p.as_str()))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_match_star() {
        assert!(matches_pattern("%SYS.Security", "^%SYS*"));
        assert!(matches_pattern("%SYSNotReal", "^%SYS*"));
        assert!(matches_pattern("%SYSOTHER", "^%SYS*"));
    }

    #[test]
    fn no_match_unrelated() {
        assert!(!matches_pattern("MySYS", "^%SYS*"));
        assert!(!matches_pattern("MyAppData", "^PAPMI*"));
    }

    #[test]
    fn exact_match_no_star() {
        assert!(matches_pattern("rOBJ", "^rOBJ"));
        assert!(!matches_pattern("rOBJExtra", "^rOBJ"));
    }

    #[test]
    fn case_insensitive() {
        assert!(matches_pattern("papmi", "^PAPMI*"));
        assert!(matches_pattern("PAPMI123", "^PAPMI*"));
    }

    #[test]
    fn phi_patterns_cover_expected_names() {
        assert!(matches_any("PAPMI", PHI_NAME_PATTERNS));
        assert!(matches_any("PAADM1234", PHI_NAME_PATTERNS));
        assert!(matches_any("ORDER123", PHI_NAME_PATTERNS));
        assert!(!matches_any("MyAppData", PHI_NAME_PATTERNS));
    }

    #[test]
    fn system_blocklist_count() {
        assert_eq!(SYSTEM_BLOCKLIST.len(), 30);
    }

    #[test]
    fn phi_patterns_count() {
        assert_eq!(PHI_NAME_PATTERNS.len(), 9);
    }
}
