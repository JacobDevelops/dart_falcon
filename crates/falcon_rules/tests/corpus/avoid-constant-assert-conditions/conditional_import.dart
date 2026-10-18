import 'environment_values.dart' if (dart.library.io) 'conditional_alternative.dart' as values;
void check() { assert(values.ordinary == 3); }
void independentLiteral() {
  assert(true); /* expect: avoid-constant-assert-conditions */
}
