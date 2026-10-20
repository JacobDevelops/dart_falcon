import 'environment_values.dart' as values;
import 'environment_values.dart' deferred as later;
void importedAssertions() {
  assert(values.ordinary == 3); /* expect: avoid-constant-assert-conditions */
  assert(values.derived);
  assert(values.present);
  assert(later.ordinary == 3);
  const captured = values.derived;
  assert(captured);
}
