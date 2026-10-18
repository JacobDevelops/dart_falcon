void objects(Object value) {
  if (value case Point(zebra: var z, apple: var a)) {} /* expect: pattern-fields-ordering */
  if (value case Point(:var zebra, :var apple)) {} /* expect: pattern-fields-ordering */
  if (value case (zebra: var z, apple: var a)) {} /* expect: pattern-fields-ordering */
  if (value case (:var zebra, :var apple)) {} /* expect: pattern-fields-ordering */
  if (value case (0, zebra: var z, apple: var a)) {} /* expect: pattern-fields-ordering */
}
class Point { final int apple = 1; final int zebra = 2; }
