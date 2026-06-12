module check.record_enum

record Point {
  x: Int
  y: Int
}

enum Shape {
  Dot
  Line { length: Int }
}

func demo(p: Point) -> Int {
  let partial = Point { x: 1 }
  let extra = Point { x: 1, y: 2, z: 3 }
  let direct = Shape { length: 1 }
  let typo = Shape.Dott
  return p.z
}
