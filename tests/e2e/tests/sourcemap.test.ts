// source map の検証(goal 条件 4): 生成 TS の契約違反 throw 行が
// .kei 側の requires 行へ解決されること。

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const here = dirname(fileURLToPath(import.meta.url));
const generated = join(here, "..", "generated");

// ---- source map v3 mappings の復号(base64 VLQ) ----

const BASE64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

function decodeSegment(segment: string): number[] {
  const fields: number[] = [];
  let value = 0;
  let shift = 0;
  for (const ch of segment) {
    const digit = BASE64.indexOf(ch);
    if (digit < 0) {
      throw new Error(`invalid VLQ character: ${ch}`);
    }
    value |= (digit & 0x1f) << shift;
    if (digit & 0x20) {
      shift += 5;
    } else {
      const negative = (value & 1) === 1;
      const magnitude = value >>> 1;
      fields.push(negative ? -magnitude : magnitude);
      value = 0;
      shift = 0;
    }
  }
  return fields;
}

type ResolvedMapping = {
  genLine: number; // 0 始まり
  genCol: number;
  srcLine: number; // 0 始まり
  srcCol: number;
};

function decodeMappings(mappings: string): ResolvedMapping[] {
  const out: ResolvedMapping[] = [];
  let srcLine = 0;
  let srcCol = 0;
  mappings.split(";").forEach((group, genLine) => {
    let genCol = 0;
    for (const segment of group.split(",")) {
      if (segment === "") {
        continue;
      }
      const fields = decodeSegment(segment);
      expect(fields.length).toBeGreaterThanOrEqual(4);
      genCol += fields[0]!;
      srcLine += fields[2]!;
      srcCol += fields[3]!;
      out.push({ genLine, genCol, srcLine, srcCol });
    }
  });
  return out;
}

/** 生成行に対応する最も左のマッピング(行頭=その文の出所)を返す。 */
function resolveLine(mappings: ResolvedMapping[], genLine: number): ResolvedMapping {
  const onLine = mappings.filter((m) => m.genLine === genLine);
  expect(onLine.length, `generated line ${genLine + 1} must be mapped`).toBeGreaterThan(0);
  return onLine.reduce((a, b) => (a.genCol <= b.genCol ? a : b));
}

describe("source map", () => {
  const ts = readFileSync(join(generated, "contracts", "withdraw.ts"), "utf8");
  const rawMap = readFileSync(join(generated, "contracts", "withdraw.ts.map"), "utf8");
  const map = JSON.parse(rawMap) as {
    version: number;
    sources: string[];
    sourcesContent: string[];
    mappings: string;
  };

  it("source map v3 として .kei ソースを参照している", () => {
    expect(map.version).toBe(3);
    expect(map.sources[0]).toBe("examples/contracts/withdraw.kei");
    expect(ts).toContain("//# sourceMappingURL=withdraw.ts.map");
  });

  it("requires 違反の throw 行が .kei の requires 行へ解決される", () => {
    const keiSource = map.sourcesContent[0]!;
    const requiresLine = keiSource
      .split("\n")
      .findIndex((l) => l.includes("requires amount > Money.zero"));
    expect(requiresLine).toBeGreaterThanOrEqual(0);

    const throwLine = ts
      .split("\n")
      .findIndex((l) => l.includes("throw new KeiContractViolation"));
    expect(throwLine).toBeGreaterThanOrEqual(0);

    const mappings = decodeMappings(map.mappings);
    const resolved = resolveLine(mappings, throwLine);
    expect(resolved.srcLine).toBe(requiresLine);
  });

  it("else fail の展開行が .kei の let 行へ解決される", () => {
    const keiSource = map.sourcesContent[0]!;
    const letLine = keiSource
      .split("\n")
      .findIndex((l) => l.includes("let current = Database.fetchBalance"));
    expect(letLine).toBeGreaterThanOrEqual(0);

    const constLine = ts
      .split("\n")
      .findIndex((l) => l.includes("const current$ = Database.fetchBalance"));
    expect(constLine).toBeGreaterThanOrEqual(0);

    const mappings = decodeMappings(map.mappings);
    const resolved = resolveLine(mappings, constLine);
    expect(resolved.srcLine).toBe(letLine);
  });
});
