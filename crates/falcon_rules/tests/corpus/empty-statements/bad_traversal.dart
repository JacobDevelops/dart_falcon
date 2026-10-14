// Empty statements in every closure position, and labeled statements.
final callbacks = [() {
  ; /* expect: empty-statements */
}];

class Example {
  final callback = () {
    ; /* expect: empty-statements */
  };
  Example() : other = (() {
    ; /* expect: empty-statements */
  });
  final other;
}

void nested(bool choose) {
  final values = [() {
    ; /* expect: empty-statements */
  }];
  final callback = choose ? () {
    ; /* expect: empty-statements */
  } : () { null; };
  if ((() {
    ; /* expect: empty-statements */
    return true;
  })()) {}
  marker: ; /* expect: empty-statements */
  for (;;) { break; }
}
