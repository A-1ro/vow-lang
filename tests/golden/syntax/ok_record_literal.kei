func makeReceipt(from: AccountId, to: AccountId, amount: Money) -> TransferReceipt {
  return TransferReceipt { from, to, amount }
}

func pendingStatus(now: Timestamp) -> Status {
  return Status.Pending { since: now, retries: 0 }
}
