module basics.records

record Point {
  x: Int
  y: Int
}

func origin() -> Point {
  return Point { x: 0, y: 0 }
}

func shift(p: Point, dx: Int, dy: Int) -> Point {
  return Point { x: p.x + dx, y: p.y + dy }
}
