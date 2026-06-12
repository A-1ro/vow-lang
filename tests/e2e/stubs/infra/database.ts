// e2e スタブ: examples が import する infra.database のインメモリ実装。
// spec §3.3 の「テスト時は DI 的なモジュール差し替えで代替」に相当する。

import { None, Option, Some } from "@pact/runtime";
import { AccountId, Money } from "../core/money";

export type Account = {
  readonly balance: Money;
};

const balances = new Map<string, Money>();

export function reset(): void {
  balances.clear();
}

export function seed(account: AccountId, balance: Money): void {
  balances.set(account, balance);
}

export function balanceOf(account: AccountId): Money | undefined {
  return balances.get(account);
}

export function fetchBalance(account: AccountId): Option<Money> {
  const balance = balances.get(account);
  return balance === undefined ? None() : Some(balance);
}

export function setBalance(account: AccountId, balance: Money): void {
  balances.set(account, balance);
}

export function fetchAccount(account: AccountId): Option<Account> {
  const balance = balances.get(account);
  return balance === undefined ? None() : Some({ balance });
}

export function debit(account: AccountId, amount: Money): void {
  balances.set(account, (balances.get(account) ?? 0) - amount);
}

export function credit(account: AccountId, amount: Money): void {
  balances.set(account, (balances.get(account) ?? 0) + amount);
}
