//! fmt テスト共通ヘルパ。AST の比較は span を除いた JSON で行う
//! (整形は位置情報を必ず変えるため、意味的同一性のみを問う)。

use serde_json::Value;

/// AST を span キー抜きの JSON 値にする。
pub fn ast_json(module: &kei_syntax::Module) -> Value {
    let mut value = serde_json::to_value(module).expect("AST is serializable");
    strip_spans(&mut value);
    value
}

fn strip_spans(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("span");
            for v in map.values_mut() {
                strip_spans(v);
            }
        }
        Value::Array(items) => {
            for v in items {
                strip_spans(v);
            }
        }
        _ => {}
    }
}
