// @dart = 3.13

extension type const Count(int value) {}

void makeCount() {
  final count = Count(1); /* expect: prefer-const-constructors */
  const constantCount = Count(1);
  print([count, constantCount]);
}

class Bounds {
  const Bounds.tightFor({int? height});
  Bounds copyWith({int? minWidth}) => this;
}

void chainedShorthand() {
  Bounds bounds = .tightFor(height: 400).copyWith(minWidth: 400); /* expect: prefer-const-constructors */
  print(bounds);
}
