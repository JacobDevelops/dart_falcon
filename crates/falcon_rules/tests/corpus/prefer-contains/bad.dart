bool missing(List<String> values, String value) {
  return values.indexOf(value) == -1; /* expect: prefer-contains */
}
bool present(String text, String value) {
  return text.indexOf(value) >= 0; /* expect: prefer-contains */
}
bool defaultValue([bool found = 'abc'.indexOf('a') >= 0]) => found; /* expect: prefer-contains */
bool swappedPresent(List<String> values, String value) {
  return -1 < values.indexOf(value); /* expect: prefer-contains */
}
bool swappedMissing(String text, String value) {
  return 0 <= text.indexOf(value); /* expect: prefer-contains */
}
bool otherForms(String text, String value) {
  if (text.indexOf(value) < 0) return false; /* expect: prefer-contains */
  if (text.indexOf(value) <= -1) return false; /* expect: prefer-contains */
  if (-1 >= text.indexOf(value)) return false; /* expect: prefer-contains */
  return text.indexOf(value) != -1; /* expect: prefer-contains */
}
bool hexBounds(String text, String value) {
  if (text.indexOf(value) != -0x1) return true; /* expect: prefer-contains */
  return text.indexOf(value) >= 0x0; /* expect: prefer-contains */
}

class SearchState {
  final bool found;
  SearchState(String text) : found = text.indexOf('x') >= 0; /* expect: prefer-contains */

  bool get fromGetter {
    String text = 'abc';
    return text.indexOf('a') >= 0; /* expect: prefer-contains */
  }

  set values(List<String> values) {
    values.indexOf('x') >= 0; /* expect: prefer-contains */
  }

  bool operator ==(Object other) {
    String text = other.toString();
    return text.indexOf('x') >= 0; /* expect: prefer-contains */
  }
}

void scoped(List<String> values, List<String> texts) {
  bool local(String text) => text.indexOf('x') >= 0; /* expect: prefer-contains */
  for (String text in texts) {
    text.indexOf('x') >= 0; /* expect: prefer-contains */
  }
  try {
    throw 'x';
  } on String catch (text) {
    text.indexOf('x') >= 0; /* expect: prefer-contains */
  }
  final (String text,) = ('abc',);
  text.indexOf('x') >= 0; /* expect: prefer-contains */
}
