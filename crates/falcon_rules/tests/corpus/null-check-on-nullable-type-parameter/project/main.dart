import 'types.dart';

U force<U>(U? value) {
  return value!; /* expect: null-check-on-nullable-type-parameter */
}

T concrete(T? value) => value!;
