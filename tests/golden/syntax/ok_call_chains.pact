func report(tx: Transfer) -> Bool
  uses Audit.Log
{
  Audit.Log.record(tx)
  let settled = tx.receipt.isOk
  return settled != false
}
