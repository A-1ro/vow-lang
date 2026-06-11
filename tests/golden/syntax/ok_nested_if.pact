func classify(n: Int) -> String {
  if n < 0 {
    return "negative"
  } else {
    if n == 0 {
      return "zero"
    } else {
      return "positive"
    }
  }
}
