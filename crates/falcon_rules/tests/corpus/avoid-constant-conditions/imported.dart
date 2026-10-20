import 'environment_values.dart' as values;
import 'environment_values.dart' deferred as later;
void importedConditions() {
  if (values.ordinary == 3) {} /* expect: avoid-constant-conditions */
  if (values.derived == true) {}
  if (values.present == true) {}
  if (later.ordinary == 3) {}
  const captured = values.derived;
  if (captured == true) {}
}
