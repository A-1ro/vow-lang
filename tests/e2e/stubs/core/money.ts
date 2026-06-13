// e2e スタブ: examples が import する core.money の TS 実装。
// Kei 側では import 先は信頼境界の外(Ty::Unknown)。実体はテストが用意する。

export type Money = number;

export const Money = {
  zero: 0 as Money,
  of(value: number): Money {
    return value;
  },
};

export type AccountId = string & { readonly __keiTag: "AccountId" };

export function AccountId(value: string): AccountId {
  return value as AccountId;
}
