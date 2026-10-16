extension NullableText on String? {
  bool get isBlank => this == null || this!.trim().isEmpty;
  String orEmpty() => this ?? '';
  int operator [](int index) => this?.codeUnitAt(index) ?? 0;
  set label(String value) {}
}

class Crate {
  const Crate(this.size);
  final int size;
}

extension NullableCrate on Crate? {
  int area() => this == null ? 0 : this!.size * this!.size;
  int operator [](int index) => index;
}

void main(String? text, Crate? box) {
  print(text?.isBlank); /* expect: unnecessary-null-aware-operator-on-extension-on-nullable */
  print(text?.orEmpty()); /* expect: unnecessary-null-aware-operator-on-extension-on-nullable */
  text?.label = 'x'; /* expect: unnecessary-null-aware-operator-on-extension-on-nullable */
  print(box?[0]); /* expect: unnecessary-null-aware-operator-on-extension-on-nullable */
  print(box?.area()); /* expect: unnecessary-null-aware-operator-on-extension-on-nullable */
  print(NullableText(text)?.orEmpty()); /* expect: unnecessary-null-aware-operator-on-extension-on-nullable */
  print(box /* ? */
      ?[0]); /* expect: unnecessary-null-aware-operator-on-extension-on-nullable */
  text /* ?. */
      ?.label = 'x'; /* expect: unnecessary-null-aware-operator-on-extension-on-nullable */
}
