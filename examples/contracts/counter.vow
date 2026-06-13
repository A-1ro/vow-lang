module contracts.counter

func increment(count: Int, step: Int) -> Int
  requires step > 0
  ensures result == old(count) + step
{
  return count + step
}
