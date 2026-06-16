// e2e スタブ: examples が import する infra.time のインメモリ実装。
// extern Time.now() -> Int uses Clock(spec/kei-spec-v0.2.md §2)に対応する。

let frozen = 0;

export function now(): number {
  return frozen;
}

export function setNow(t: number): void {
  frozen = t;
}
