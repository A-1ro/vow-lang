//! roundtrip プロパティテスト(M2 完了条件)。
//!
//! - examples/ 配下の全 .pact: `parse(fmt(parse(src))) == parse(src)`(span 除去比較)
//!   かつ `fmt(fmt(x)) == fmt(x)`(冪等性)
//! - proptest 生成 AST 1000 件: `parse(format_module(ast)) == ast`(span 除去比較)。
//!   これは生成ソース `src = format_module(ast)` に対する
//!   `parse(fmt(parse(src))) == parse(src)` を含意するより強い性質。

mod util;

use proptest::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

use pact_syntax::ast::*;
use pact_syntax::{Position, Span};

// ---- examples/ roundtrip ----

fn collect_pact_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
    {
        let path = entry.expect("readable dir entry").path();
        if path.is_dir() {
            collect_pact_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "pact") {
            out.push(path);
        }
    }
}

#[test]
fn examples_roundtrip_and_idempotent() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut files = Vec::new();
    collect_pact_files(&dir, &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .pact files in {}", dir.display());

    for path in &files {
        let src = fs::read_to_string(path).expect("readable example");
        let parsed = pact_syntax::parse_module(&src);
        assert!(
            parsed.errors.is_empty(),
            "{}: example must parse cleanly: {:?}",
            path.display(),
            parsed.errors
        );

        let formatted = pact_fmt::format_module(&parsed.module);
        let reparsed = pact_syntax::parse_module(&formatted);
        assert!(
            reparsed.errors.is_empty(),
            "{}: formatted output must parse cleanly: {:?}\n--- formatted ---\n{}",
            path.display(),
            reparsed.errors,
            formatted
        );
        // parse(fmt(parse(src))) == parse(src)
        assert_eq!(
            util::ast_json(&parsed.module),
            util::ast_json(&reparsed.module),
            "{}: formatting changed the AST",
            path.display()
        );
        // fmt(fmt(x)) == fmt(x)
        assert_eq!(
            formatted,
            pact_fmt::format_module(&reparsed.module),
            "{}: formatting is not idempotent",
            path.display()
        );
    }
}

// ---- proptest 用 AST 生成 ----

fn sp() -> Span {
    Span::point(Position::new(1, 1))
}

const KEYWORDS: &[&str] = &[
    "module", "import", "as", "type", "record", "enum", "func", "uses", "requires", "ensures",
    "let", "if", "else", "fail", "return", "tagged", "true", "false", "implies",
];

fn ident() -> impl Strategy<Value = Ident> {
    "[a-z][a-z0-9]{0,5}"
        .prop_filter("not a keyword", |s| !KEYWORDS.contains(&s.as_str()))
        .prop_map(|name| Ident { name, span: sp() })
}

fn path1() -> impl Strategy<Value = Vec<Ident>> {
    prop::collection::vec(ident(), 1..=3)
}

fn str_value() -> impl Strategy<Value = String> {
    // 印字可能 ASCII。`"` と `\` はフォーマッタのエスケープを通す。
    "[ -~]{0,8}"
}

fn ty() -> impl Strategy<Value = Type> {
    let leaf = path1().prop_map(|path| Type {
        path,
        args: Vec::new(),
        span: sp(),
    });
    leaf.prop_recursive(2, 8, 2, |inner| {
        (path1(), prop::collection::vec(inner, 1..=2)).prop_map(|(path, args)| Type {
            path,
            args,
            span: sp(),
        })
    })
}

fn bin_op() -> impl Strategy<Value = BinOp> {
    prop_oneof![
        Just(BinOp::Eq),
        Just(BinOp::Ne),
        Just(BinOp::Lt),
        Just(BinOp::Gt),
        Just(BinOp::Le),
        Just(BinOp::Ge),
        Just(BinOp::Add),
        Just(BinOp::Sub),
        Just(BinOp::Mul),
        Just(BinOp::Div),
        Just(BinOp::Implies),
    ]
}

fn expr() -> impl Strategy<Value = Expr> {
    let leaf = prop_oneof![
        (0..=i64::MAX).prop_map(|value| Expr::Int { value, span: sp() }),
        str_value().prop_map(|value| Expr::Str { value, span: sp() }),
        any::<bool>().prop_map(|value| Expr::Bool { value, span: sp() }),
        ident().prop_map(|i| Expr::Name {
            name: i.name,
            span: sp()
        }),
    ];
    leaf.prop_recursive(3, 24, 3, |inner| {
        let lit_field =
            (ident(), prop::option::of(inner.clone())).prop_map(|(name, value)| RecordLitField {
                name,
                value,
                span: sp(),
            });
        prop_oneof![
            (inner.clone(), ident()).prop_map(|(base, name)| Expr::Field {
                base: Box::new(base),
                name,
                span: sp()
            }),
            (inner.clone(), prop::collection::vec(inner.clone(), 0..=2)).prop_map(
                |(callee, args)| Expr::Call {
                    callee: Box::new(callee),
                    args,
                    span: sp()
                }
            ),
            (
                prop_oneof![Just(UnaryOp::Neg), Just(UnaryOp::Not)],
                inner.clone()
            )
                .prop_map(|(op, expr)| Expr::Unary {
                    op,
                    expr: Box::new(expr),
                    span: sp()
                }),
            (bin_op(), inner.clone(), inner.clone()).prop_map(|(op, lhs, rhs)| Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span: sp()
            }),
            (path1(), prop::collection::vec(lit_field, 0..=3)).prop_map(|(path, fields)| {
                Expr::RecordLit {
                    path,
                    fields,
                    span: sp(),
                }
            }),
        ]
    })
}

fn let_stmt() -> impl Strategy<Value = Stmt> {
    (
        ident(),
        prop::option::of(ty()),
        expr(),
        prop::option::of(expr()),
    )
        .prop_map(|(name, ty, value, else_fail)| {
            Stmt::Let(LetStmt {
                name,
                ty,
                value,
                else_fail,
                span: sp(),
            })
        })
}

fn stmt() -> impl Strategy<Value = Stmt> {
    let leaf = prop_oneof![
        let_stmt(),
        prop::option::of(expr()).prop_map(|value| Stmt::Return(ReturnStmt { value, span: sp() })),
        expr().prop_map(|expr| Stmt::Expr(ExprStmt { expr, span: sp() })),
    ];
    leaf.prop_recursive(2, 12, 3, |inner| {
        let block =
            prop::collection::vec(inner, 0..=3).prop_map(|stmts| Block { stmts, span: sp() });
        let else_branch = prop_oneof![
            block.clone().prop_map(ElseBranch::Block),
            (expr(), block.clone()).prop_map(|(cond, then_block)| {
                ElseBranch::If(Box::new(IfStmt {
                    cond,
                    then_block,
                    else_branch: None,
                    span: sp(),
                }))
            }),
        ];
        (expr(), block, prop::option::of(else_branch)).prop_map(
            |(cond, then_block, else_branch)| {
                Stmt::If(IfStmt {
                    cond,
                    then_block,
                    else_branch,
                    span: sp(),
                })
            },
        )
    })
}

fn block() -> impl Strategy<Value = Block> {
    prop::collection::vec(stmt(), 0..=4).prop_map(|stmts| Block { stmts, span: sp() })
}

fn field_def() -> impl Strategy<Value = FieldDef> {
    (ident(), ty()).prop_map(|(name, ty)| FieldDef {
        name,
        ty,
        span: sp(),
    })
}

fn variant() -> impl Strategy<Value = Variant> {
    let payload = prop_oneof![
        Just(VariantPayload::Unit),
        prop::collection::vec(ty(), 0..=2).prop_map(|types| VariantPayload::Tuple { types }),
        prop::collection::vec(field_def(), 0..=2)
            .prop_map(|fields| VariantPayload::Record { fields }),
    ];
    (ident(), payload).prop_map(|(name, payload)| Variant {
        name,
        payload,
        span: sp(),
    })
}

fn func() -> impl Strategy<Value = Item> {
    let param = (ident(), ty()).prop_map(|(name, ty)| Param {
        name,
        ty,
        span: sp(),
    });
    let effect = path1().prop_map(|path| EffectRef { path, span: sp() });
    (
        ident(),
        prop::collection::vec(param, 0..=3),
        prop::option::of(ty()),
        prop::collection::vec(effect, 0..=2),
        prop::collection::vec(expr(), 0..=2),
        prop::collection::vec(expr(), 0..=2),
        block(),
    )
        .prop_map(|(name, params, ret, uses, requires, ensures, body)| {
            Item::Func(FuncDecl {
                name,
                params,
                ret,
                uses,
                requires,
                ensures,
                body,
                span: sp(),
            })
        })
}

fn item() -> impl Strategy<Value = Item> {
    prop_oneof![
        (ident(), ty(), prop::option::of(str_value())).prop_map(|(name, ty, tag)| {
            Item::TypeAlias(TypeAlias {
                name,
                ty,
                tag,
                span: sp(),
            })
        }),
        (ident(), prop::collection::vec(field_def(), 0..=3)).prop_map(|(name, fields)| {
            Item::Record(RecordDecl {
                name,
                fields,
                span: sp(),
            })
        }),
        (ident(), prop::collection::vec(variant(), 0..=3)).prop_map(|(name, variants)| {
            Item::Enum(EnumDecl {
                name,
                variants,
                span: sp(),
            })
        }),
        func(),
    ]
}

fn import() -> impl Strategy<Value = Import> {
    let tail = prop_oneof![
        Just((Vec::new(), None)),
        prop::collection::vec(ident(), 1..=3).prop_map(|names| (names, None)),
        ident().prop_map(|alias| (Vec::new(), Some(alias))),
    ];
    (path1(), tail).prop_map(|(path, (names, alias))| Import {
        path,
        names,
        alias,
        span: sp(),
    })
}

fn module() -> impl Strategy<Value = Module> {
    (
        prop::option::of(path1().prop_map(|path| ModuleDecl { path, span: sp() })),
        prop::collection::vec(import(), 0..=3),
        prop::collection::vec(item(), 0..=4),
    )
        .prop_map(|(decl, imports, items)| Module {
            decl,
            imports,
            items,
            span: sp(),
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// 生成 AST → fmt → parse が(span を除いて)元の AST に戻り、
    /// かつ fmt が冪等であること。
    #[test]
    fn generated_ast_roundtrips(m in module()) {
        let src = pact_fmt::format_module(&m);
        let parsed = pact_syntax::parse_module(&src);
        prop_assert!(
            parsed.errors.is_empty(),
            "formatted output must parse cleanly: {:?}\n--- src ---\n{}",
            parsed.errors,
            src
        );
        prop_assert_eq!(
            util::ast_json(&m),
            util::ast_json(&parsed.module),
            "formatting must preserve the AST\n--- src ---\n{}",
            src
        );
        let again = pact_fmt::format_module(&parsed.module);
        prop_assert_eq!(&src, &again, "fmt(fmt(x)) must equal fmt(x)");
    }
}
