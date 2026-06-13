//! Kei MCP サーバー。spec/ と examples/ をビルド時に埋め込み(ARCHITECTURE.md
//! 不変条件 2)、エージェント向けの取扱説明書として配信する。
//!
//! 言語処理ロジックは持たず、検査・整形は kei_check / kei_fmt / kei_syntax に
//! 委譲する。プロトコル処理は [`Server::handle`] が担い、stdio トランスポートは
//! `src/main.rs` がこれを薄く包む。

pub mod embedded;
pub mod server;
pub mod tools;

pub use server::Server;
