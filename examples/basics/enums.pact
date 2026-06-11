module basics.enums

type OrderId = String tagged "OrderId"

enum OrderStatus {
  Draft
  Submitted(OrderId)
  Rejected { reason: String, retryable: Bool }
}

func statusCode(submitted: Bool, rejected: Bool) -> Int {
  if submitted {
    return 1
  } else if rejected {
    return 2
  } else {
    return 0
  }
}
