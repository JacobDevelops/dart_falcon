class Deprecated {
  const Deprecated([String message = '']);
}
class Marker {
  const Marker();
}
const deprecated = Marker();

@deprecated
void first() {}

@Deprecated()
void second() {}

@Deprecated('')
void third() {}
