func transferFunds(from: AccountId, to: AccountId, amount: Money) -> Result<TransferReceipt, TransferError>
  uses Database.Write, Audit.Log
  requires amount > Money.zero
  requires from != to
  ensures result.isOk implies balanceOf(from) == old(balanceOf(from)) - amount
{
  let sender = Database.fetchAccount(from) else fail TransferError.NotFound(from)
  if sender.balance < amount {
    return Err(TransferError.InsufficientFunds { needed: amount, had: sender.balance })
  }
  Database.debit(from, amount)
  Database.credit(to, amount)
  Audit.Log.record(Transfer { from, to, amount })
  return Ok(TransferReceipt { from, to, amount })
}
