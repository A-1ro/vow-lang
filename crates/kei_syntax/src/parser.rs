//! 再帰下降パーサ。エラー回復に対応し、1 回のパースで複数の
//! [`SyntaxError`] を収集する。エラーがあっても可能な範囲の AST を返す。
//!
//! 回復方針:
//! - 宣言レベル: 次の宣言開始キーワード(func / record / enum / type / import / module)
//!   までスキップ
//! - 文レベル: 次の改行または `}` までスキップ
//! - 単一トークンの欠落(`)` `:` 等)は挿入されたとみなして続行する

use crate::ast::*;
use crate::error::{codes, FixHint, SyntaxError};
use crate::span::Span;
use crate::token::{Token, TokenKind as T};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    errors: Vec<SyntaxError>,
}

/// 契約節キーワード(typo 検出 KEI-E0104 の照合対象)。
const CLAUSE_KEYWORDS: [&str; 3] = ["uses", "requires", "ensures"];

impl Parser {
    pub fn new(tokens: Vec<Token>, lex_errors: Vec<SyntaxError>) -> Self {
        Self {
            tokens,
            pos: 0,
            errors: lex_errors,
        }
    }

    pub fn into_errors(self) -> Vec<SyntaxError> {
        self.errors
    }

    // ---- トークン操作 ----

    fn cur(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn at(&self, kind: T) -> bool {
        self.cur().kind == kind
    }

    fn bump(&mut self) -> Token {
        let tok = self.cur().clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    fn eat(&mut self, kind: T) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn skip_newlines(&mut self) {
        while self.at(T::Newline) {
            self.bump();
        }
    }

    /// 改行を読み飛ばした先が `kind` なら読み飛ばしを確定して true。
    /// そうでなければ位置を戻して false。
    fn at_after_newlines(&mut self, kind: T) -> bool {
        let save = self.pos;
        self.skip_newlines();
        if self.at(kind) {
            true
        } else {
            self.pos = save;
            false
        }
    }

    // ---- エラー報告 ----

    fn error(&mut self, code: &'static str, message: String, span: Span, fix: FixHint) {
        self.errors.push(SyntaxError {
            code,
            message,
            span,
            fix,
        });
    }

    /// `kind` を要求する。欠落時は挿入 fix つきの KEI-E0101 を報告して false。
    fn expect(&mut self, kind: T) -> bool {
        if self.eat(kind) {
            return true;
        }
        let found = self.cur().clone();
        let lit = kind.literal().unwrap_or(kind.describe());
        let fix = FixHint::replace(
            format!("Insert '{lit}'"),
            Span::point(found.span.start),
            lit,
        );
        self.error(
            codes::UNEXPECTED_TOKEN,
            format!("expected '{lit}', found {}", found.found_label()),
            found.span,
            fix,
        );
        false
    }

    /// 識別子を要求する。予約語が来た場合は KEI-E0102 を報告した上で
    /// その綴りを識別子として採用する(回復)。
    fn expect_ident(&mut self, what: &str) -> Option<Ident> {
        let tok = self.cur().clone();
        match tok.kind {
            T::Ident => {
                self.bump();
                Some(Ident {
                    name: tok.text,
                    span: tok.span,
                })
            }
            k if k.is_keyword() => {
                let fix = FixHint::replace(
                    format!("Rename to '{}_'", tok.text),
                    tok.span,
                    format!("{}_", tok.text),
                );
                self.error(
                    codes::RESERVED_IDENT,
                    format!(
                        "'{}' is a reserved keyword and cannot be used as {what}",
                        tok.text
                    ),
                    tok.span,
                    fix,
                );
                self.bump();
                Some(Ident {
                    name: tok.text,
                    span: tok.span,
                })
            }
            _ => {
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    format!("expected {what}, found {}", tok.found_label()),
                    tok.span,
                    FixHint::direction(format!("Write {what} here")),
                );
                None
            }
        }
    }

    /// `.` の後ろのメンバー名。キーワードと同綴りの名前も許す
    /// (文脈キーワード。例: `Audit.Log.record(...)` — spec §2.1)。
    fn expect_member_ident(&mut self) -> Option<Ident> {
        let tok = self.cur().clone();
        if tok.kind == T::Ident || tok.kind.is_keyword() {
            self.bump();
            return Some(Ident {
                name: tok.text,
                span: tok.span,
            });
        }
        self.error(
            codes::UNEXPECTED_TOKEN,
            format!("expected a field name, found {}", tok.found_label()),
            tok.span,
            FixHint::direction("Write a field name here"),
        );
        None
    }

    fn unclosed_delimiter(&mut self, open: &str, open_span: Span, close: &str) {
        let here = self.cur().span;
        self.error(
            codes::UNCLOSED_DELIMITER,
            format!("unclosed '{open}': missing '{close}' before end of file"),
            open_span,
            FixHint::replace(format!("Insert '{close}'"), Span::point(here.start), close),
        );
    }

    // ---- 回復 ----

    /// 次の宣言開始キーワード(ブレース深度 0)または EOF までスキップ。
    fn recover_to_decl(&mut self) {
        let mut depth: u32 = 0;
        loop {
            match self.cur().kind {
                T::Eof => return,
                T::Module | T::Import | T::Type | T::Record | T::Enum | T::Func if depth == 0 => {
                    return;
                }
                T::LBrace => {
                    depth += 1;
                    self.bump();
                }
                T::RBrace => {
                    depth = depth.saturating_sub(1);
                    self.bump();
                }
                _ => {
                    self.bump();
                }
            }
        }
    }

    /// 文の終わり(改行・`}`・EOF)までスキップ。ネストしたブレースは飛ばす。
    fn recover_to_stmt_end(&mut self) {
        let mut depth: u32 = 0;
        loop {
            match self.cur().kind {
                T::Eof => return,
                T::Newline if depth == 0 => {
                    self.bump();
                    self.skip_newlines();
                    return;
                }
                T::RBrace if depth == 0 => return,
                T::LBrace => {
                    depth += 1;
                    self.bump();
                }
                T::RBrace => {
                    depth = depth.saturating_sub(1);
                    self.bump();
                }
                _ => {
                    self.bump();
                }
            }
        }
    }

    /// フィールド・バリアント並びの中での回復: 改行・`,`・`}`・EOF まで。
    fn recover_in_braces(&mut self) {
        loop {
            match self.cur().kind {
                T::Eof | T::Newline | T::Comma | T::RBrace => return,
                _ => {
                    self.bump();
                }
            }
        }
    }

    // ---- モジュール ----

    pub fn parse_module(&mut self) -> Module {
        let start = self.cur().span;
        let mut decl = None;
        let mut imports = Vec::new();
        let mut items = Vec::new();

        self.skip_newlines();
        if self.at(T::Module) {
            decl = self.parse_module_decl();
        }

        loop {
            self.skip_newlines();
            match self.cur().kind {
                T::Eof => break,
                T::Import => {
                    if let Some(import) = self.parse_import() {
                        imports.push(import);
                    }
                }
                T::Type => {
                    if let Some(item) = self.parse_type_alias() {
                        items.push(Item::TypeAlias(item));
                    }
                }
                T::Record => {
                    if let Some(item) = self.parse_record() {
                        items.push(Item::Record(item));
                    }
                }
                T::Enum => {
                    if let Some(item) = self.parse_enum() {
                        items.push(Item::Enum(item));
                    }
                }
                T::Func => {
                    if let Some(item) = self.parse_func() {
                        items.push(Item::Func(item));
                    }
                }
                T::Module => {
                    let tok = self.cur().clone();
                    self.error(
                        codes::UNEXPECTED_TOKEN,
                        "duplicate 'module' declaration: a file declares its module at most once"
                            .to_string(),
                        tok.span,
                        FixHint::direction("Remove this 'module' declaration"),
                    );
                    self.bump();
                    self.recover_to_decl();
                }
                _ => {
                    let tok = self.cur().clone();
                    self.error(
                        codes::UNEXPECTED_TOKEN,
                        format!(
                            "expected a declaration (func, record, enum, type, or import), found {}",
                            tok.found_label()
                        ),
                        tok.span,
                        FixHint::direction(
                            "Start a declaration with 'func', 'record', 'enum', 'type', or 'import'",
                        ),
                    );
                    self.bump();
                    self.recover_to_decl();
                }
            }
        }

        let end = self.cur().span;
        Module {
            decl,
            imports,
            items,
            span: start.to(end),
        }
    }

    fn parse_path(&mut self, what: &str) -> Option<Vec<Ident>> {
        let mut path = vec![self.expect_ident(what)?];
        while self.eat(T::Dot) {
            path.push(self.expect_ident(what)?);
        }
        Some(path)
    }

    fn parse_module_decl(&mut self) -> Option<ModuleDecl> {
        let kw = self.bump();
        let path = match self.parse_path("a module path segment") {
            Some(path) => path,
            None => {
                self.recover_to_decl();
                return None;
            }
        };
        let end = path.last().map(|i| i.span).unwrap_or(kw.span);
        Some(ModuleDecl {
            path,
            span: kw.span.to(end),
        })
    }

    fn parse_import(&mut self) -> Option<Import> {
        let kw = self.bump();
        let path = match self.parse_path("a module path segment") {
            Some(path) => path,
            None => {
                self.recover_to_decl();
                return None;
            }
        };
        let mut names = Vec::new();
        let mut alias = None;
        let mut end = path.last().map(|i| i.span).unwrap_or(kw.span);

        if self.at(T::LBrace) {
            let open = self.bump();
            self.skip_newlines();
            while !self.at(T::RBrace) {
                if self.at(T::Eof) {
                    self.unclosed_delimiter("{", open.span, "}");
                    break;
                }
                match self.expect_ident("an imported name") {
                    Some(name) => names.push(name),
                    None => {
                        self.recover_in_braces();
                    }
                }
                self.eat(T::Comma);
                self.skip_newlines();
            }
            end = self.cur().span;
            self.eat(T::RBrace);
        } else if self.eat(T::As) {
            alias = self.expect_ident("an import alias");
            if let Some(a) = &alias {
                end = a.span;
            }
        }

        Some(Import {
            path,
            names,
            alias,
            span: kw.span.to(end),
        })
    }

    // ---- 型宣言 ----

    fn parse_type_alias(&mut self) -> Option<TypeAlias> {
        let kw = self.bump();
        let name = match self.expect_ident("a type name") {
            Some(name) => name,
            None => {
                self.recover_to_decl();
                return None;
            }
        };
        if !self.expect(T::Eq) && !self.at(T::Ident) {
            self.recover_to_decl();
            return None;
        }
        let ty = match self.parse_type() {
            Some(ty) => ty,
            None => {
                self.recover_to_decl();
                return None;
            }
        };
        let mut tag = None;
        let mut end = ty.span;
        if self.eat(T::Tagged) {
            if self.at(T::Str) {
                let tok = self.bump();
                end = tok.span;
                tag = Some(tok.text);
            } else {
                let tok = self.cur().clone();
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    format!(
                        "expected a string literal after 'tagged', found {}",
                        tok.found_label()
                    ),
                    tok.span,
                    FixHint::direction(
                        "Write the tag as a string literal, e.g. tagged \"AccountId\"",
                    ),
                );
            }
        }
        Some(TypeAlias {
            name,
            ty,
            tag,
            span: kw.span.to(end),
        })
    }

    fn parse_type(&mut self) -> Option<Type> {
        let first = self.expect_ident("a type name")?;
        let mut path = vec![first];
        while self.eat(T::Dot) {
            path.push(self.expect_ident("a type name")?);
        }
        let mut args = Vec::new();
        let mut end = path.last().expect("path is non-empty").span;
        if self.eat(T::Lt) {
            loop {
                args.push(self.parse_type()?);
                if !self.eat(T::Comma) {
                    break;
                }
            }
            end = self.cur().span;
            self.expect(T::Gt);
        }
        let start = path.first().expect("path is non-empty").span;
        Some(Type {
            path,
            args,
            span: start.to(end),
        })
    }

    // ---- record / enum ----

    /// `{ name: Type ... }` 形式のフィールド並び(改行・カンマ区切り)を読む。
    /// 開きブレースは消費済みであること。戻り値は閉じブレース直後の span。
    fn parse_field_list(&mut self, open_span: Span, fields: &mut Vec<FieldDef>) -> Span {
        loop {
            self.skip_newlines();
            if self.at(T::RBrace) {
                let close = self.bump();
                return close.span;
            }
            if self.at(T::Eof) {
                self.unclosed_delimiter("{", open_span, "}");
                return self.cur().span;
            }
            let name = match self.expect_ident("a field name") {
                Some(name) => name,
                None => {
                    self.recover_in_braces();
                    self.eat(T::Comma);
                    continue;
                }
            };
            if !self.expect(T::Colon) && !self.at(T::Ident) {
                self.recover_in_braces();
                self.eat(T::Comma);
                continue;
            }
            let ty = match self.parse_type() {
                Some(ty) => ty,
                None => {
                    self.recover_in_braces();
                    self.eat(T::Comma);
                    continue;
                }
            };
            let span = name.span.to(ty.span);
            fields.push(FieldDef { name, ty, span });
            self.eat(T::Comma);
        }
    }

    fn parse_record(&mut self) -> Option<RecordDecl> {
        let kw = self.bump();
        let name = match self.expect_ident("a record name") {
            Some(name) => name,
            None => {
                self.recover_to_decl();
                return None;
            }
        };
        if !self.at(T::LBrace) {
            self.expect(T::LBrace);
            self.recover_to_decl();
            return None;
        }
        let open = self.bump();
        let mut fields = Vec::new();
        let end = self.parse_field_list(open.span, &mut fields);
        Some(RecordDecl {
            name,
            fields,
            span: kw.span.to(end),
        })
    }

    fn parse_enum(&mut self) -> Option<EnumDecl> {
        let kw = self.bump();
        let name = match self.expect_ident("an enum name") {
            Some(name) => name,
            None => {
                self.recover_to_decl();
                return None;
            }
        };
        if !self.at(T::LBrace) {
            self.expect(T::LBrace);
            self.recover_to_decl();
            return None;
        }
        let open = self.bump();
        let mut variants = Vec::new();
        let end_span;
        loop {
            self.skip_newlines();
            if self.at(T::RBrace) {
                end_span = self.bump().span;
                break;
            }
            if self.at(T::Eof) {
                self.unclosed_delimiter("{", open.span, "}");
                end_span = self.cur().span;
                break;
            }
            match self.parse_variant() {
                Some(variant) => variants.push(variant),
                None => {
                    self.recover_in_braces();
                }
            }
            self.eat(T::Comma);
        }
        Some(EnumDecl {
            name,
            variants,
            span: kw.span.to(end_span),
        })
    }

    fn parse_variant(&mut self) -> Option<Variant> {
        let name = self.expect_ident("a variant name")?;
        let mut end = name.span;
        let payload = if self.at(T::LParen) {
            let open = self.bump();
            let mut types = Vec::new();
            loop {
                self.skip_newlines();
                if self.at(T::RParen) {
                    break;
                }
                if self.at(T::Eof) {
                    self.unclosed_delimiter("(", open.span, ")");
                    break;
                }
                match self.parse_type() {
                    Some(ty) => types.push(ty),
                    None => {
                        self.recover_in_braces();
                        break;
                    }
                }
                if !self.eat(T::Comma) {
                    break;
                }
            }
            end = self.cur().span;
            self.expect(T::RParen);
            VariantPayload::Tuple { types }
        } else if self.at(T::LBrace) {
            let open = self.bump();
            let mut fields = Vec::new();
            end = self.parse_field_list(open.span, &mut fields);
            VariantPayload::Record { fields }
        } else {
            VariantPayload::Unit
        };
        let span = name.span.to(end);
        Some(Variant {
            name,
            payload,
            span,
        })
    }

    // ---- func ----

    fn parse_func(&mut self) -> Option<FuncDecl> {
        let kw = self.bump();
        let name = match self.expect_ident("a function name") {
            Some(name) => name,
            None => {
                self.recover_to_decl();
                return None;
            }
        };
        if !self.expect(T::LParen) {
            self.recover_to_decl();
            return None;
        }
        let params = self.parse_params();

        let mut ret = None;
        if self.at_after_newlines(T::Arrow) {
            self.bump();
            ret = self.parse_type();
            if ret.is_none() {
                self.recover_to_stmt_end();
            }
        }

        let mut uses = Vec::new();
        let mut requires = Vec::new();
        let mut ensures = Vec::new();
        loop {
            let save = self.pos;
            self.skip_newlines();
            match self.cur().kind {
                T::Uses => {
                    self.bump();
                    self.parse_effect_list(&mut uses);
                }
                T::Requires => {
                    self.bump();
                    self.parse_clause_expr(&mut requires);
                }
                T::Ensures => {
                    self.bump();
                    self.parse_clause_expr(&mut ensures);
                }
                T::Ident => {
                    let tok = self.cur().clone();
                    match nearest_clause_keyword(&tok.text) {
                        Some(kw_text) => {
                            self.error(
                                codes::UNKNOWN_CLAUSE,
                                format!(
                                    "unknown clause '{}': contract clauses are 'uses', 'requires', and 'ensures'",
                                    tok.text
                                ),
                                tok.span,
                                FixHint::replace(
                                    format!("Replace '{}' with '{kw_text}'", tok.text),
                                    tok.span,
                                    kw_text,
                                ),
                            );
                            self.bump();
                            match kw_text {
                                "uses" => self.parse_effect_list(&mut uses),
                                "requires" => self.parse_clause_expr(&mut requires),
                                _ => self.parse_clause_expr(&mut ensures),
                            }
                        }
                        None => {
                            self.pos = save;
                            break;
                        }
                    }
                }
                _ => {
                    if !self.at(T::LBrace) {
                        self.pos = save;
                    }
                    break;
                }
            }
        }

        self.skip_newlines();
        let body = if self.at(T::LBrace) {
            self.parse_block()
        } else {
            let tok = self.cur().clone();
            self.error(
                codes::UNEXPECTED_TOKEN,
                format!(
                    "expected '{{' to start the function body, found {}",
                    tok.found_label()
                ),
                tok.span,
                FixHint::replace("Insert '{'", Span::point(tok.span.start), "{"),
            );
            self.recover_to_decl();
            Block {
                stmts: Vec::new(),
                span: Span::point(tok.span.start),
            }
        };

        let span = kw.span.to(body.span);
        Some(FuncDecl {
            name,
            params,
            ret,
            uses,
            requires,
            ensures,
            body,
            span,
        })
    }

    fn parse_params(&mut self) -> Vec<Param> {
        let open_span = self.tokens[self.pos - 1].span;
        let mut params = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(T::RParen) {
                self.bump();
                return params;
            }
            if self.at(T::Eof) {
                self.unclosed_delimiter("(", open_span, ")");
                return params;
            }
            // 「: が続かない宣言開始キーワード」は次の宣言の始まり。
            // '(' が閉じられていないとみなして引数並びを打ち切る。
            if matches!(
                self.cur().kind,
                T::Module | T::Import | T::Type | T::Record | T::Enum | T::Func
            ) && self.tokens.get(self.pos + 1).map(|t| t.kind) != Some(T::Colon)
            {
                self.unclosed_delimiter("(", open_span, ")");
                return params;
            }
            let name = match self.expect_ident("a parameter name") {
                Some(name) => name,
                None => {
                    self.recover_in_parens();
                    continue;
                }
            };
            if !self.expect(T::Colon) && !self.at(T::Ident) {
                self.recover_in_parens();
                continue;
            }
            let ty = match self.parse_type() {
                Some(ty) => ty,
                None => {
                    self.recover_in_parens();
                    continue;
                }
            };
            let span = name.span.to(ty.span);
            params.push(Param { name, ty, span });
            self.skip_newlines();
            match self.cur().kind {
                T::Comma => {
                    self.bump();
                }
                T::RParen => {}
                _ => {
                    // `)` の欠落とみなして引数並びを終える(挿入 fix を提示)
                    let tok = self.cur().clone();
                    self.error(
                        codes::UNEXPECTED_TOKEN,
                        format!(
                            "expected ',' or ')' in parameter list, found {}",
                            tok.found_label()
                        ),
                        tok.span,
                        FixHint::replace("Insert ')'", Span::point(tok.span.start), ")"),
                    );
                    return params;
                }
            }
        }
    }

    /// 引数・パラメータ並びの中での回復: `,` `)` 改行 EOF まで。
    fn recover_in_parens(&mut self) {
        loop {
            match self.cur().kind {
                T::Eof | T::Newline | T::Comma | T::RParen => return,
                _ => {
                    self.bump();
                }
            }
        }
    }

    fn parse_effect_list(&mut self, uses: &mut Vec<EffectRef>) {
        loop {
            match self.parse_path("an effect name") {
                Some(path) => {
                    let start = path.first().expect("path is non-empty").span;
                    let end = path.last().expect("path is non-empty").span;
                    uses.push(EffectRef {
                        path,
                        span: start.to(end),
                    });
                }
                None => {
                    self.recover_to_stmt_end();
                    return;
                }
            }
            if !self.eat(T::Comma) {
                return;
            }
        }
    }

    fn parse_clause_expr(&mut self, out: &mut Vec<Expr>) {
        match self.parse_expr(false) {
            Some(expr) => out.push(expr),
            None => self.recover_to_stmt_end(),
        }
    }

    // ---- 文 ----

    fn parse_block(&mut self) -> Block {
        let open = self.bump(); // '{'(呼び出し側で確認済み)
        let mut stmts = Vec::new();
        let end_span;
        loop {
            self.skip_newlines();
            if self.at(T::RBrace) {
                end_span = self.bump().span;
                break;
            }
            if self.at(T::Eof) {
                self.unclosed_delimiter("{", open.span, "}");
                end_span = self.cur().span;
                break;
            }
            match self.parse_stmt() {
                Some(stmt) => {
                    stmts.push(stmt);
                    self.expect_stmt_end();
                }
                None => {
                    self.recover_to_stmt_end();
                }
            }
        }
        Block {
            stmts,
            span: open.span.to(end_span),
        }
    }

    fn expect_stmt_end(&mut self) {
        match self.cur().kind {
            T::Newline => {
                self.bump();
                self.skip_newlines();
            }
            T::RBrace | T::Eof => {}
            _ => {
                let tok = self.cur().clone();
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    format!(
                        "expected a line break after this statement, found {}",
                        tok.found_label()
                    ),
                    tok.span,
                    FixHint::replace("Insert a line break", Span::point(tok.span.start), "\n"),
                );
                self.recover_to_stmt_end();
            }
        }
    }

    fn parse_stmt(&mut self) -> Option<Stmt> {
        match self.cur().kind {
            T::Let => self.parse_let().map(Stmt::Let),
            T::If => self.parse_if().map(Stmt::If),
            T::Return => self.parse_return().map(Stmt::Return),
            _ => {
                let expr = self.parse_expr(false)?;
                let span = expr.span();
                Some(Stmt::Expr(ExprStmt { expr, span }))
            }
        }
    }

    fn parse_let(&mut self) -> Option<LetStmt> {
        let kw = self.bump();
        let name = self.expect_ident("a variable name")?;
        let ty = if self.eat(T::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        if !self.expect(T::Eq) && !is_expr_start(self.cur().kind) {
            return None;
        }
        let value = self.parse_expr(false)?;
        let mut end = value.span();
        let mut else_fail = None;
        if self.at(T::Else) {
            self.bump();
            self.expect(T::Fail);
            let fail_expr = self.parse_expr(false)?;
            end = fail_expr.span();
            else_fail = Some(fail_expr);
        }
        Some(LetStmt {
            name,
            ty,
            value,
            else_fail,
            span: kw.span.to(end),
        })
    }

    fn parse_if(&mut self) -> Option<IfStmt> {
        let kw = self.bump();
        let cond = self.parse_expr(true)?;
        if !self.at(T::LBrace) {
            let tok = self.cur().clone();
            self.error(
                codes::UNEXPECTED_TOKEN,
                format!(
                    "expected '{{' after the if condition, found {}",
                    tok.found_label()
                ),
                tok.span,
                FixHint::replace("Insert '{'", Span::point(tok.span.start), "{"),
            );
            return None;
        }
        let then_block = self.parse_block();
        let mut end = then_block.span;
        let mut else_branch = None;
        if self.at_after_newlines(T::Else) {
            self.bump();
            if self.at(T::If) {
                let nested = self.parse_if()?;
                end = nested.span;
                else_branch = Some(ElseBranch::If(Box::new(nested)));
            } else if self.at(T::LBrace) {
                let block = self.parse_block();
                end = block.span;
                else_branch = Some(ElseBranch::Block(block));
            } else {
                let tok = self.cur().clone();
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    format!(
                        "expected 'if' or '{{' after 'else', found {}",
                        tok.found_label()
                    ),
                    tok.span,
                    FixHint::replace("Insert '{'", Span::point(tok.span.start), "{"),
                );
            }
        }
        Some(IfStmt {
            cond,
            then_block,
            else_branch,
            span: kw.span.to(end),
        })
    }

    fn parse_return(&mut self) -> Option<ReturnStmt> {
        let kw = self.bump();
        let mut span = kw.span;
        let value = match self.cur().kind {
            T::Newline | T::RBrace | T::Eof => None,
            _ => {
                let expr = self.parse_expr(false)?;
                span = kw.span.to(expr.span());
                Some(expr)
            }
        };
        Some(ReturnStmt { value, span })
    }

    // ---- 式 ----

    fn parse_expr(&mut self, no_struct: bool) -> Option<Expr> {
        self.parse_implies(no_struct)
    }

    fn parse_implies(&mut self, no_struct: bool) -> Option<Expr> {
        let lhs = self.parse_cmp(no_struct)?;
        if self.at(T::Implies) {
            self.bump();
            // 右結合
            let rhs = self.parse_implies(no_struct)?;
            let span = lhs.span().to(rhs.span());
            return Some(Expr::Binary {
                op: BinOp::Implies,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            });
        }
        Some(lhs)
    }

    fn parse_cmp(&mut self, no_struct: bool) -> Option<Expr> {
        let mut lhs = self.parse_add(no_struct)?;
        loop {
            let op = match self.cur().kind {
                T::EqEq => BinOp::Eq,
                T::NotEq => BinOp::Ne,
                T::Lt => BinOp::Lt,
                T::Gt => BinOp::Gt,
                T::Le => BinOp::Le,
                T::Ge => BinOp::Ge,
                _ => return Some(lhs),
            };
            self.bump();
            let rhs = self.parse_add(no_struct)?;
            let span = lhs.span().to(rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
    }

    fn parse_add(&mut self, no_struct: bool) -> Option<Expr> {
        let mut lhs = self.parse_mul(no_struct)?;
        loop {
            let op = match self.cur().kind {
                T::Plus => BinOp::Add,
                T::Minus => BinOp::Sub,
                _ => return Some(lhs),
            };
            self.bump();
            let rhs = self.parse_mul(no_struct)?;
            let span = lhs.span().to(rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
    }

    fn parse_mul(&mut self, no_struct: bool) -> Option<Expr> {
        let mut lhs = self.parse_unary(no_struct)?;
        loop {
            let op = match self.cur().kind {
                T::Star => BinOp::Mul,
                T::Slash => BinOp::Div,
                _ => return Some(lhs),
            };
            self.bump();
            let rhs = self.parse_unary(no_struct)?;
            let span = lhs.span().to(rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
    }

    fn parse_unary(&mut self, no_struct: bool) -> Option<Expr> {
        let op = match self.cur().kind {
            T::Minus => UnaryOp::Neg,
            T::Bang => UnaryOp::Not,
            _ => return self.parse_postfix(no_struct),
        };
        let tok = self.bump();
        let expr = self.parse_unary(no_struct)?;
        let span = tok.span.to(expr.span());
        Some(Expr::Unary {
            op,
            expr: Box::new(expr),
            span,
        })
    }

    fn parse_postfix(&mut self, no_struct: bool) -> Option<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.cur().kind {
                T::Dot => {
                    self.bump();
                    let name = self.expect_member_ident()?;
                    let span = expr.span().to(name.span);
                    expr = Expr::Field {
                        base: Box::new(expr),
                        name,
                        span,
                    };
                }
                T::LParen => {
                    let open = self.bump();
                    let mut args = Vec::new();
                    loop {
                        self.skip_newlines();
                        if self.at(T::RParen) {
                            break;
                        }
                        if self.at(T::Eof) {
                            self.unclosed_delimiter("(", open.span, ")");
                            break;
                        }
                        args.push(self.parse_expr(false)?);
                        self.skip_newlines();
                        if !self.eat(T::Comma) {
                            break;
                        }
                    }
                    let close = self.cur().span;
                    self.expect(T::RParen);
                    let span = expr.span().to(close);
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        args,
                        span,
                    };
                }
                T::LBrace if !no_struct => {
                    let Some(path) = path_of(&expr) else {
                        return Some(expr);
                    };
                    expr = self.parse_record_lit(path)?;
                }
                _ => return Some(expr),
            }
        }
    }

    fn parse_record_lit(&mut self, path: Vec<Ident>) -> Option<Expr> {
        let open = self.bump(); // '{'
        let start = path.first().expect("path is non-empty").span;
        let mut fields = Vec::new();
        let end;
        loop {
            self.skip_newlines();
            if self.at(T::RBrace) {
                end = self.bump().span;
                break;
            }
            if self.at(T::Eof) {
                self.unclosed_delimiter("{", open.span, "}");
                end = self.cur().span;
                break;
            }
            let name = match self.expect_ident("a field name") {
                Some(name) => name,
                None => {
                    self.recover_in_braces();
                    self.eat(T::Comma);
                    continue;
                }
            };
            let mut span = name.span;
            let value = if self.eat(T::Colon) {
                let expr = self.parse_expr(false)?;
                span = span.to(expr.span());
                Some(expr)
            } else {
                None
            };
            fields.push(RecordLitField { name, value, span });
            self.skip_newlines();
            if !self.eat(T::Comma) && !self.at(T::RBrace) && !self.at(T::Eof) {
                let tok = self.cur().clone();
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    format!(
                        "expected ',' or '}}' in record literal, found {}",
                        tok.found_label()
                    ),
                    tok.span,
                    FixHint::replace("Insert ','", Span::point(tok.span.start), ","),
                );
                self.recover_in_braces();
            }
        }
        Some(Expr::RecordLit {
            path,
            fields,
            span: start.to(end),
        })
    }

    fn parse_primary(&mut self) -> Option<Expr> {
        let tok = self.cur().clone();
        match tok.kind {
            T::Int => {
                self.bump();
                Some(Expr::Int {
                    value: tok.text.parse().unwrap_or(0),
                    span: tok.span,
                })
            }
            T::Str => {
                self.bump();
                Some(Expr::Str {
                    value: tok.text,
                    span: tok.span,
                })
            }
            T::True | T::False => {
                self.bump();
                Some(Expr::Bool {
                    value: tok.kind == T::True,
                    span: tok.span,
                })
            }
            T::Ident => {
                self.bump();
                Some(Expr::Name {
                    name: tok.text,
                    span: tok.span,
                })
            }
            T::LParen => {
                self.bump();
                let expr = self.parse_expr(false)?;
                self.expect(T::RParen);
                Some(expr)
            }
            // 文を開始できない予約語は「識別子のつもり」とみなして回復する
            T::Type
            | T::Record
            | T::Enum
            | T::Module
            | T::Import
            | T::Tagged
            | T::As
            | T::Uses
            | T::Requires
            | T::Ensures => {
                let fix = FixHint::replace(
                    format!("Rename to '{}_'", tok.text),
                    tok.span,
                    format!("{}_", tok.text),
                );
                self.error(
                    codes::RESERVED_IDENT,
                    format!(
                        "'{}' is a reserved keyword and cannot be used as an expression",
                        tok.text
                    ),
                    tok.span,
                    fix,
                );
                self.bump();
                Some(Expr::Name {
                    name: tok.text,
                    span: tok.span,
                })
            }
            _ => {
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    format!("expected an expression, found {}", tok.found_label()),
                    tok.span,
                    FixHint::direction("Write an expression here"),
                );
                None
            }
        }
    }
}

/// 式を開始できるトークンか。
fn is_expr_start(kind: T) -> bool {
    matches!(
        kind,
        T::Ident | T::Int | T::Str | T::True | T::False | T::LParen | T::Minus | T::Bang
    )
}

/// 純粋なパス形(`A.B.C`)の式なら識別子列に戻す。record リテラルの名前判定に使う。
fn path_of(expr: &Expr) -> Option<Vec<Ident>> {
    match expr {
        Expr::Name { name, span } => Some(vec![Ident {
            name: name.clone(),
            span: *span,
        }]),
        Expr::Field { base, name, .. } => {
            let mut path = path_of(base)?;
            path.push(name.clone());
            Some(path)
        }
        _ => None,
    }
}

/// 契約節キーワードへの編集距離が 2 以下なら最も近いものを返す(typo 検出)。
fn nearest_clause_keyword(text: &str) -> Option<&'static str> {
    CLAUSE_KEYWORDS
        .iter()
        .map(|kw| (levenshtein(text, kw), *kw))
        .filter(|(d, _)| *d <= 2)
        .min_by_key(|(d, _)| *d)
        .map(|(_, kw)| kw)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}
