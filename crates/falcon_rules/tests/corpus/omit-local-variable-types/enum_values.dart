enum PlatformChoice { first, second }

enum NumericChoice<T extends num> {
  integer<int>(1), decimal<double>(1.5);
  const NumericChoice(this.value);
  final T value;
}

class UserValues {
  static const List<String> values = <String>['user'];
}

void enumValues(int? result, PlatformChoice previous) {
  List<PlatformChoice> choices = PlatformChoice.values; /* expect: omit-local-variable-types */
  PlatformChoice selected = PlatformChoice.values[result!]; /* expect: omit-local-variable-types */
  int position = PlatformChoice.values.indexOf(previous); /* expect: omit-local-variable-types */
  for (final PlatformChoice candidate in PlatformChoice.values) { /* expect: omit-local-variable-types */
    PlatformChoice copy = candidate; /* expect: omit-local-variable-types */
    print(copy);
  }
  for (final candidate in PlatformChoice.values) {
    PlatformChoice copy = candidate; /* expect: omit-local-variable-types */
    print(copy);
  }
  final collected = [for (final candidate in PlatformChoice.values) candidate];
  List<PlatformChoice> copied = collected; /* expect: omit-local-variable-types */
  List<NumericChoice<num>> numeric = NumericChoice.values; /* expect: omit-local-variable-types */
  NumericChoice<num> number = NumericChoice.values[0]; /* expect: omit-local-variable-types */
  List<String> userValues = UserValues.values; /* expect: omit-local-variable-types */
  Iterable<PlatformChoice> widened = PlatformChoice.values;
  PlatformChoice? nullable = PlatformChoice.values[0];
  Object values = PlatformChoice.values;
  print([choices, selected, position, copied, numeric, number, userValues, widened, nullable, values]);
}
