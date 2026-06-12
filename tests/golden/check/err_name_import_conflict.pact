module check.import_conflict

import core.money { Money }
import billing.money { Money }

record Money {
  amount: Int
}
