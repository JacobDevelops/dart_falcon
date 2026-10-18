void access(List<int> values, Map<int, String> names, dynamic unknown) {
  print(names[0]);
  print(values.firstOrNull);
  print(values.lastOrNull);
  print(values.singleOrNull);
  print(values.firstWhere((value) => value > 0, orElse: () => 0));
  print(values.lastWhere((value) => value > 0, orElse: () => 0));
  print(values.singleWhere((value) => value > 0, orElse: () => 0));
  print(unknown.first);
}
class Custom {
  int get first => 0;
  int operator [](int index) => 0;
}
void custom(Custom value) { print(value.first); print(value[0]); }


class Splittable {
  Custom split(String separator) => Custom();
}

void customSplit(Splittable value) {
  print(value.split(',').first);
}

void customPatterns(Object value) {
  if (value case Custom(first: final first)) {
    print(first);
  }
}

extension LocalExtrema on Iterable<int> {
  int get max => 0;
  int get min => 0;
}

void localExtrema(Iterable<int> values) {
  print(values.max);
  print(values.min);
  if (values case Iterable<int>(max: final maximum)) {
    print(maximum);
  }
}
