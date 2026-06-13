//! source map v3 の生成。mappings は [genCol, srcIdx, srcLine, srcCol] の
//! 4 要素セグメントを base64 VLQ で相対符号化したもの(単一ソース前提)。

/// 1 マッピング。全フィールド 0 始まり。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mapping {
    pub gen_line: u32,
    pub gen_col: u32,
    pub src_line: u32,
    pub src_col: u32,
}

const BASE64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn vlq_encode(value: i64, out: &mut String) {
    // 符号ビットを最下位へ。
    let mut v: u64 = if value < 0 {
        (((-value) as u64) << 1) | 1
    } else {
        (value as u64) << 1
    };
    loop {
        let mut digit = (v & 0x1f) as usize;
        v >>= 5;
        if v != 0 {
            digit |= 0x20;
        }
        out.push(BASE64[digit] as char);
        if v == 0 {
            break;
        }
    }
}

/// マッピング列(gen_line, gen_col 昇順)を mappings 文字列にする。
pub fn encode_mappings(mappings: &[Mapping]) -> String {
    let mut out = String::new();
    let mut cur_line = 0u32;
    let mut prev_gen_col = 0i64;
    let mut prev_src_line = 0i64;
    let mut prev_src_col = 0i64;
    let mut first_in_line = true;
    for m in mappings {
        while cur_line < m.gen_line {
            out.push(';');
            cur_line += 1;
            prev_gen_col = 0;
            first_in_line = true;
        }
        if !first_in_line {
            out.push(',');
        }
        vlq_encode(i64::from(m.gen_col) - prev_gen_col, &mut out);
        vlq_encode(0, &mut out); // sources は常に 1 本(インデックス 0)
        vlq_encode(i64::from(m.src_line) - prev_src_line, &mut out);
        vlq_encode(i64::from(m.src_col) - prev_src_col, &mut out);
        prev_gen_col = i64::from(m.gen_col);
        prev_src_line = i64::from(m.src_line);
        prev_src_col = i64::from(m.src_col);
        first_in_line = false;
    }
    out
}

/// source map v3 の JSON 全体を構築する。
pub fn build_map(ts_file: &str, src_file: &str, source: &str, mappings: &[Mapping]) -> String {
    let map = serde_json::json!({
        "version": 3,
        "file": ts_file,
        "sources": [src_file],
        "sourcesContent": [source],
        "names": [],
        "mappings": encode_mappings(mappings),
    });
    let mut out = serde_json::to_string_pretty(&map).expect("source map is serializable");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vlq_basics() {
        let mut s = String::new();
        vlq_encode(0, &mut s);
        assert_eq!(s, "A");
        let mut s = String::new();
        vlq_encode(16, &mut s);
        assert_eq!(s, "gB");
        let mut s = String::new();
        vlq_encode(-1, &mut s);
        assert_eq!(s, "D");
    }

    #[test]
    fn encode_relative_segments() {
        let ms = [
            Mapping {
                gen_line: 0,
                gen_col: 0,
                src_line: 0,
                src_col: 0,
            },
            Mapping {
                gen_line: 2,
                gen_col: 4,
                src_line: 1,
                src_col: 2,
            },
        ];
        assert_eq!(encode_mappings(&ms), "AAAA;;IACE");
    }
}
