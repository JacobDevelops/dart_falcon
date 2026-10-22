class Deprecated {
  const Deprecated(this.message);
  final String message;
}
class Marker {
  const Marker();
}
const deprecated = Marker();

@deprecated
void first() {}

@Deprecated('Local annotation, not SDK deprecation')
void second() {}

void use() {
  first();
  second();
}
