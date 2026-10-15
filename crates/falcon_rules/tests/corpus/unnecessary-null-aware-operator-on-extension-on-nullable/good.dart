extension NullableText on String? {
  bool get isBlank => this == null || this!.trim().isEmpty;
  int get length => 0;
  int operator [](int index) => 0;
}

extension NonNullText on String {
  String shout() => toUpperCase();
}

class Box {
  const Box(this.size);
  final int size;
  int area() => size * size;
}

extension NullableBox on Box? {
  int area() => 0;
}

extension MaybeNum on num? {
  bool get isPositive => this != null && this! > 0;
}

extension NonNullInt on int {
  bool get isPositive => this > 0;
}

void main(String? text, Box? box, int? count) {
  print(text.isBlank);
  print(text?.shout());
  print(box?.area());
  print(text?.length);
  print(count?.isPositive);
  print(NonNullText(text ?? '').shout());
  // `String` declares `length` and `[]`; instance members win over extensions.
  print(text?.length);
  print(text?[0]);
}
