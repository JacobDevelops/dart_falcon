int readCount() => 3;
var top = readCount(); /* expect: prefer-type-over-var */
class Box {
  var count = readCount(); /* expect: prefer-type-over-var */
}
void main() {
  var count = readCount(); /* expect: prefer-type-over-var */
  var first = readCount(), second = readCount(); /* expect: prefer-type-over-var */ /* expect: prefer-type-over-var */
  var number = 3; /* expect: prefer-type-over-var */
  var values = <int>[]; /* expect: prefer-type-over-var */
  var unset; /* expect: prefer-type-over-var */
}
