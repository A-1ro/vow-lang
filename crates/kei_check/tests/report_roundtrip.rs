//! CheckReport / ContractInfo / Verification の JSON シリアライズ roundtrip テスト
//! (M12)。スキーマは spec/diagnostic-schema.md が正。

use kei_check::{
    CheckReport, ContractInfo, ContractKind, Diagnostic, Fix, Position, Severity, Span, TextEdit,
    Verification,
};

fn span(line: u32, col: u32) -> Span {
    Span {
        file: "demo.kei".to_string(),
        start: Position { line, col },
        end: Position { line, col: col + 5 },
    }
}

fn sample_report() -> CheckReport {
    let diag = Diagnostic::new(
        Severity::Error,
        "KEI-E3001",
        "effect 'Clock' used but not declared",
        span(10, 3),
        vec![Fix {
            title: "Add 'Clock' to uses clause".to_string(),
            edits: vec![TextEdit {
                span: span(2, 21),
                new_text: ", Clock".to_string(),
            }],
        }],
    )
    .expect("fixes non-empty");
    CheckReport {
        diagnostics: vec![diag],
        contracts: vec![
            ContractInfo {
                func: "increment".to_string(),
                kind: ContractKind::Requires,
                expr: "step > 0".to_string(),
                verification: Verification::Runtime,
                span: span(4, 12),
            },
            ContractInfo {
                func: "increment".to_string(),
                kind: ContractKind::Ensures,
                expr: "true".to_string(),
                verification: Verification::Static,
                span: span(5, 11),
            },
        ],
    }
}

#[test]
fn roundtrip_preserves_report() {
    let original = sample_report();
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: CheckReport = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
}

#[test]
fn report_has_diagnostics_and_contracts_keys() {
    let value = serde_json::to_value(sample_report()).unwrap();
    let obj = value.as_object().unwrap();
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["contracts", "diagnostics"]);

    let c = &value["contracts"][0];
    let mut ckeys: Vec<&str> = c.as_object().unwrap().keys().map(String::as_str).collect();
    ckeys.sort_unstable();
    assert_eq!(ckeys, ["expr", "func", "kind", "span", "verification"]);
}

#[test]
fn kind_and_verification_serialize_as_lowercase() {
    assert_eq!(
        serde_json::to_string(&ContractKind::Requires).unwrap(),
        "\"requires\""
    );
    assert_eq!(
        serde_json::to_string(&ContractKind::Ensures).unwrap(),
        "\"ensures\""
    );
    for (v, s) in [
        (Verification::Static, "\"static\""),
        (Verification::Runtime, "\"runtime\""),
        (Verification::Trusted, "\"trusted\""),
        (Verification::Unchecked, "\"unchecked\""),
    ] {
        assert_eq!(serde_json::to_string(&v).unwrap(), s);
    }
}

/// 前方互換: 未知フィールドは読み捨て可(スキーマのシリアライズ規約)。
#[test]
fn ignores_unknown_contract_fields() {
    let mut value = serde_json::to_value(sample_report()).unwrap();
    value["contracts"][0]["future_field"] = serde_json::json!("ignored");
    let parsed: CheckReport = serde_json::from_value(value).expect("deserialize");
    assert_eq!(parsed, sample_report());
}

/// spec/diagnostic-schema.md の CheckReport 例がそのまま読めること。
#[test]
fn deserializes_schema_example() {
    let json = r#"{
        "diagnostics": [],
        "contracts": [
            {
                "func": "increment",
                "kind": "requires",
                "expr": "step > 0",
                "verification": "runtime",
                "span": {
                    "file": "demo.kei",
                    "start": { "line": 4, "col": 12 },
                    "end": { "line": 4, "col": 20 }
                }
            }
        ]
    }"#;
    let parsed: CheckReport = serde_json::from_str(json).expect("deserialize schema example");
    assert_eq!(parsed.contracts.len(), 1);
    assert_eq!(parsed.contracts[0].verification, Verification::Runtime);
    assert_eq!(parsed.contracts[0].kind, ContractKind::Requires);
}
