// e2e スタブ: examples が import する infra.audit のインメモリ実装。

const entries: unknown[] = [];

export const Log = {
  record(entry: unknown): void {
    entries.push(entry);
  },
};

export function reset(): void {
  entries.length = 0;
}

export function logEntries(): readonly unknown[] {
  return entries;
}
