// Catch clauses in every closure position are checked.
final callbacks = [() {
  try { work(); } catch (error) {} /* expect: empty-catches */
}];

class Example {
  final callback = () {
    try { work(); } catch (error) {} /* expect: empty-catches */
  };
  Example() : other = (() {
    try { work(); } catch (error) {} /* expect: empty-catches */
  });
  final other;
}

void nested(bool choose) {
  final values = [() {
    try { work(); } catch (error) {} /* expect: empty-catches */
  }];
  final callback = choose ? () {
    try { work(); } catch (error) {} /* expect: empty-catches */
  } : () {
    try { work(); } catch (_) {}
  };
  if ((() {
    try { work(); } catch (error) {} /* expect: empty-catches */
    return true;
  })()) { work(); }
  final documented = () { try { work(); } catch (error) { /* intentional */ } };
}

void work() {}
