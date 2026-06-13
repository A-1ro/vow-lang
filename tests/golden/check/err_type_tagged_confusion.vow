module check.tagged_confusion

type AccountId = String tagged "AccountId"
type OrderId = String tagged "OrderId"

func exists(id: AccountId) -> Bool {
  return true
}

func demo(account: AccountId, order: OrderId, raw: String) -> Bool {
  let fromRaw = exists(raw)
  let fromOrder = exists(order)
  return account == raw
}
