abstract class Shape<T> {
  static const canonical = Empty();
  const factory Shape.empty() = Empty;
}

class Empty implements Shape<Never> {
  const Empty();
}

void use() {
  const Shape.empty(); /* expect: use-named-constants */
  const Shape<String>.empty(); /* expect: use-named-constants */
}
