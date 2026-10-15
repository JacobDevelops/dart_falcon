extension type Text(String value) implements Object {
  int indexOf(String search) => value.indexOf(search);
}

extension type Values(Iterable<String> value) {
  int indexOf(String search) => value.toList().indexOf(search);
}

extension type Counter(int value) {
  int indexOf(String search) => value;
}

bool textContains(Text text) {
  return text.indexOf('x') != -1; /* expect: prefer-contains */
}

bool valuesContain(Values values) {
  return values.indexOf('x') != -1; /* expect: prefer-contains */
}

bool boundedTextContains<T extends Text>(T text) {
  return text.indexOf('x') != -1; /* expect: prefer-contains */
}

bool counterContains(Counter counter) {
  return counter.indexOf('x') != -1;
}
