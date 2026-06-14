// Kei 言語サーバー(kei-lsp)を起動して VS Code に繋ぐ LanguageClient。
//
// 言語処理ロジックは一切持たない。`kei-lsp` バイナリを stdio で spawn し、
// 診断(publishDiagnostics)と契約 Hover をエディタへ橋渡しするだけの薄い配線。
// ビルドステップを避けるため TypeScript ではなくプレーン JS(CommonJS)で書く
// (拡張の依存方針: README「開発」節を参照)。
"use strict";

const { workspace, window, commands } = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

/** @type {LanguageClient | undefined} */
let client;

/** 設定 `kei.server.path` があればそれ、無ければ PATH 上の `kei-lsp`。 */
function resolveServerCommand() {
  const configured = workspace.getConfiguration("kei").get("server.path");
  if (typeof configured === "string" && configured.trim().length > 0) {
    return configured.trim();
  }
  return "kei-lsp";
}

function buildClient() {
  const command = resolveServerCommand();
  // run / debug とも同じ: kei-lsp は stdio で JSON-RPC を話す同期サーバー。
  const serverOptions = {
    run: { command, transport: TransportKind.stdio },
    debug: { command, transport: TransportKind.stdio },
  };
  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "kei" }],
  };
  return new LanguageClient("kei", "Kei Language Server", serverOptions, clientOptions);
}

async function startClient() {
  client = buildClient();
  try {
    await client.start();
  } catch (err) {
    client = undefined;
    window.showErrorMessage(
      `Kei 言語サーバー(kei-lsp)を起動できませんでした: ${err}. ` +
        "`cargo build -p kei_lsp` でビルドするか、設定 `kei.server.path` に実行ファイルのパスを指定してください。"
    );
  }
}

async function restartClient() {
  if (client) {
    await client.stop();
    client = undefined;
  }
  await startClient();
}

/** @param {import("vscode").ExtensionContext} context */
function activate(context) {
  context.subscriptions.push(commands.registerCommand("kei.restartServer", restartClient));
  startClient();
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
