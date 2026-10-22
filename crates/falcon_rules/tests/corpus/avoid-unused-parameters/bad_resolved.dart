class Marker {
  const Marker();
}
const override = Marker();

class Example {
  @override
  void method(int unused) {} /* expect: avoid-unused-parameters */
}
