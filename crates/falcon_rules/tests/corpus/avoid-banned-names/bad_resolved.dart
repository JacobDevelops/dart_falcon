class Marker {
  const Marker();
}
const override = Marker();

class Example {
  @override
  void legacyMethod() {} /* expect: avoid-banned-names */
}
