class Marker {
  const Marker();
}
const override = Marker();
class Base {
  void method() {}
}
class Derived extends Base {
  @override
  void method() => super.method();
}
