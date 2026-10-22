class Deprecated {
  const Deprecated(this.message);
  final String message;
}
class Marker {
  const Marker();
}
const deprecated = Marker();

class Example {
  Example(@deprecated int unused, @Deprecated('message') int other); /* expect: avoid-unused-constructor-parameters */ /* expect: avoid-unused-constructor-parameters */
}
