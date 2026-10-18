import 'environment_values.dart' if (dart.library.io) 'conditional_alternative.dart' as values;
void check() { if (values.ordinary == 3) {} }
void independentLiteral() {
  if (1 == 1) {} /* expect: avoid-constant-conditions */
}
