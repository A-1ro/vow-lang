//! Diagnostic の JSON シリアライズ roundtrip テスト。
//! スキーマは spec/diagnostic-schema.md が正。

use kei_check::{Diagnostic, Fix, Position, Severity, Span, TextEdit};

fn sample_span() -> Span {
    Span {
        file: "transfer.kei".to_string(),
        start: Position { line: 12, col: 3 },
        end: Position { line: 12, col: 28 },
    }
}

fn sample_diagnostic() -> Diagnostic {
    Diagnostic::new(
        Severity::Error,
        "KEI-E3042",
        "Effect 'Database.Write' used but not declared in 'uses' clause",
        sample_span(),
        vec![Fix {
            title: "Add 'Database.Write' to uses clause".to_string(),
            edits: vec![TextEdit {
                span: Span {
                    file: "transfer.kei".to_string(),
                    start: Position { line: 3, col: 21 },
                    end: Position { line: 3, col: 21 },
                },
                new_text: ", Database.Write".to_string(),
            }],
        }],
    )
    .expect("fixes is non-empty")
}

#[test]
fn roundtrip_preserves_diagnostic() {
    let original = sample_diagnostic();
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: Diagnostic = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
}

#[test]
fn roundtrip_preserves_diagnostic_array() {
    let mut warning = sample_diagnostic();
    warning.severity = Severity::Warning;
    warning.code = "KEI-E3001".to_string();
    let original = vec![sample_diagnostic(), warning];

    let json = serde_json::to_string(&original).expect("serialize");
    let restored: Vec<Diagnostic> = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
}

#[test]
fn severity_serializes_as_lowercase_string() {
    assert_eq!(
        serde_json::to_string(&Severity::Error).unwrap(),
        "\"error\""
    );
    assert_eq!(
        serde_json::to_string(&Severity::Warning).unwrap(),
        "\"warning\""
    );
    assert_eq!(serde_json::to_string(&Severity::Info).unwrap(), "\"info\"");
}

/// spec/diagnostic-schema.md の JSON 例がそのまま読めることの検証。
#[test]
fn deserializes_schema_example() {
    let json = r#"{
        "severity": "error",
        "code": "KEI-E3042",
        "message": "Effect 'Database.Write' used but not declared in 'uses' clause",
        "span": {
            "file": "transfer.kei",
            "start": { "line": 12, "col": 3 },
            "end": { "line": 12, "col": 28 }
        },
        "fixes": [
            {
                "title": "Add 'Database.Write' to uses clause",
                "edits": [
                    {
                        "span": {
                            "file": "transfer.kei",
                            "start": { "line": 3, "col": 21 },
                            "end": { "line": 3, "col": 21 }
                        },
                        "new_text": ", Database.Write"
                    }
                ]
            }
        ]
    }"#;

    let parsed: Diagnostic = serde_json::from_str(json).expect("deserialize schema example");
    assert_eq!(parsed, sample_diagnostic());
}

/// 前方互換: 未知フィールドは読み捨て可(スキーマのシリアライズ規約)。
#[test]
fn ignores_unknown_fields() {
    let mut value = serde_json::to_value(sample_diagnostic()).unwrap();
    value["future_field"] = serde_json::json!("ignored");
    let parsed: Diagnostic = serde_json::from_value(value).expect("deserialize");
    assert_eq!(parsed, sample_diagnostic());
}

/// 不変条件の入口検査: fix 候補ゼロの Diagnostic は構築できない。
#[test]
fn new_rejects_empty_fixes() {
    let result = Diagnostic::new(
        Severity::Error,
        "KEI-E3042",
        "some message",
        sample_span(),
        vec![],
    );
    assert!(result.is_none());
}

/// フィールド名が snake_case でスキーマ通りに出力されることの検証。
#[test]
fn serializes_expected_field_names() {
    let value = serde_json::to_value(sample_diagnostic()).unwrap();
    let obj = value.as_object().unwrap();
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["code", "fixes", "message", "severity", "span"]);

    let edit = &value["fixes"][0]["edits"][0];
    assert!(edit.get("new_text").is_some());
    assert_eq!(value["span"]["start"]["line"], 12);
}
