import 'environment_values.dart' if (dart.library.io) 'conditional_alternative.dart' as values;
void check() { switch (values.ordinary) { case 3: break; default: break; } }
void independentLiteral() {
  switch (1) { /* expect: avoid-constant-switches */
    case 1: break;
    default: break;
  }
}
