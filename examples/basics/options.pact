module basics.options

func firstPositive(a: Int, b: Int) -> Option<Int> {
  if a > 0 {
    return Some(a)
  }
  if b > 0 {
    return Some(b)
  }
  return None()
}
