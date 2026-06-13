module check.imports

import core.money { Money }
import infra.database as Database

func double(amount: Money) -> Money {
  return amount + amount
}

func ping() -> Bool {
  let status = Database.ping()
  return status.alive
}
