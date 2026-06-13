// @kei/runtime — Kei が生成した TypeScript の実行時サポート。
//
// Result / Option は Kei 組み込み型のトランスパイル先(spec §5)。
// 両者は内部判別子 `ok` を共有し、生成コードの `else fail` は
// Option / Result のどちらに対しても `.ok` で分岐できる。

export type Ok<T> = {
  readonly ok: true;
  readonly isOk: true;
  readonly isErr: false;
  readonly value: T;
};

export type Err<E> = {
  readonly ok: false;
  readonly isOk: false;
  readonly isErr: true;
  readonly error: E;
};

export type Result<T, E> = Ok<T> | Err<E>;

export function Ok<T>(value: T): Ok<T> {
  return { ok: true, isOk: true, isErr: false, value };
}

export function Err<E>(error: E): Err<E> {
  return { ok: false, isOk: false, isErr: true, error };
}

export type Some<T> = {
  readonly ok: true;
  readonly isSome: true;
  readonly isNone: false;
  readonly value: T;
};

export type None = {
  readonly ok: false;
  readonly isSome: false;
  readonly isNone: true;
};

export type Option<T> = Some<T> | None;

const NONE: None = { ok: false, isSome: false, isNone: true };

export function Some<T>(value: T): Some<T> {
  return { ok: true, isSome: true, isNone: false, value };
}

export function None(): None {
  return NONE;
}

// ---- 契約アサーション(spec §4: requires / ensures は実行時アサーション) ----

export interface ContractViolationInfo {
  /** 違反した契約節の種別。 */
  readonly clause: "requires" | "ensures";
  /** 契約を宣言していた Kei 関数名。 */
  readonly func: string;
  /** 違反した契約式(Kei ソース表記)。 */
  readonly condition: string;
  /** 契約節が書かれている .kei ファイル(リポジトリルートからの相対パス)。 */
  readonly file: string;
  /** 契約節の開始位置(1 始まり)。 */
  readonly line: number;
  readonly col: number;
}

/** 契約違反の構造化エラー。生成コードの requires / ensures 検査が送出する。 */
export class KeiContractViolation extends Error {
  readonly clause: "requires" | "ensures";
  readonly func: string;
  readonly condition: string;
  readonly file: string;
  readonly line: number;
  readonly col: number;

  constructor(info: ContractViolationInfo) {
    super(
      `${info.clause} violated in '${info.func}': ${info.condition} (${info.file}:${info.line}:${info.col})`,
    );
    this.name = "KeiContractViolation";
    this.clause = info.clause;
    this.func = info.func;
    this.condition = info.condition;
    this.file = info.file;
    this.line = info.line;
    this.col = info.col;
  }

  /** 構造化データとしての契約違反(JSON が正、散文は派生)。 */
  toJSON(): ContractViolationInfo & { readonly name: string } {
    return {
      name: this.name,
      clause: this.clause,
      func: this.func,
      condition: this.condition,
      file: this.file,
      line: this.line,
      col: this.col,
    };
  }
}
