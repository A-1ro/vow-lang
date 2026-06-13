func build(from: AccountId, to: AccountId, amount: Money) -> Transfer {
  let t = Transfer {
    from,
    to,
    amount: amount
  }
  if (Status { ok: true }).ok {
    return t
  }
  return t
}
