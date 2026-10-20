import 'environment_values.dart' as values;
import 'environment_values.dart' deferred as later;
void importedSwitches() {
  switch (values.ordinary) { /* expect: avoid-constant-switches */
    case 3: break;
    default: break;
  }
  final derived = switch (values.derived) { true => 1, false => 0 };
  final present = switch (values.present) { true => 1, false => 0 };
  final deferred = switch (later.ordinary) { 3 => 1, _ => 0 };
  const captured = values.derived;
  final local = switch (captured) { true => 1, false => 0 };
  print([derived, present, deferred, local]);
}
