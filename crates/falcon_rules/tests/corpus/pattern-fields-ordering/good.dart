void objects(Object value) {
  if (value case Point(apple: var a, zebra: var z)) {}
  if (value case Point(:var apple, :var zebra)) {}
  if (value case (apple: var a, zebra: var z)) {}
  if (value case (:var apple, :var zebra)) {}
  if (value case (var zebra, var apple)) {}
  if (value case (0, apple: var a, zebra: var z)) {}
  if (value case Point(apple: var zebra, zebra: var apple)) {}
}
class Point { final int apple = 1; final int zebra = 2; }
