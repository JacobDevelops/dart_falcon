void top({int zebra = 0, int /* expect: parameters-ordering */ alpha = 0}) {}

class Example {
  Example({int zebra = 0, int /* expect: parameters-ordering */ alpha = 0});
  void method({int zebra = 0, int /* expect: parameters-ordering */ alpha = 0}) {}
}

void closures() {
  final callback = ({int zebra = 0, int /* expect: parameters-ordering */ alpha = 0}) {};
  void local({int zebra = 0, int /* expect: parameters-ordering */ alpha = 0}) {}
}

typedef Callback = void Function({int zebra, int /* expect: parameters-ordering */ alpha});
