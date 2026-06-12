//! 標準エフェクト階層(spec §3.2)と包含判定。
//!
//! v0.1 ではユーザー定義エフェクトはなく、`uses` 節に書けるのはこの階層の
//! ノードのみ。`IO` は階層全体の根で、宣言すると全エフェクトの包括許可になる
//! (spec §3.2 が「雑だが合法」と定めるため warning にもしない — §9 未決事項 3)。

/// 標準エフェクト階層の全ノード(中間ノード含む)。
pub const STANDARD_EFFECTS: &[&str] = &[
    "IO",
    "Network",
    "Network.Read",
    "Network.Write",
    "File",
    "File.Read",
    "File.Write",
    "Database",
    "Database.Read",
    "Database.Write",
    "Clock",
    "Random",
    "Audit",
    "Audit.Log",
];

/// `path` が標準エフェクト階層のノードか。
pub fn is_known(path: &str) -> bool {
    STANDARD_EFFECTS.contains(&path)
}

/// 宣言 `declared` が使用 `used` を包含するか。
/// 自分自身・祖先ノードが包含し、`IO` は根として全エフェクトを包含する。
pub fn covers(declared: &str, used: &str) -> bool {
    if declared == used || declared == "IO" {
        return true;
    }
    used.len() > declared.len()
        && used.starts_with(declared)
        && used.as_bytes()[declared.len()] == b'.'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchy_containment() {
        assert!(covers("IO", "Database.Write"));
        assert!(covers("IO", "Clock"));
        assert!(covers("Database", "Database.Write"));
        assert!(covers("Database.Write", "Database.Write"));
        assert!(!covers("Database.Write", "Database"));
        assert!(!covers("Database.Read", "Database.Write"));
        assert!(!covers("Database", "DatabaseX"));
        assert!(!covers("Clock", "IO"));
    }
}
