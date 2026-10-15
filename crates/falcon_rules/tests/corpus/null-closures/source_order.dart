class Timer {
  static void run(void Function()? callback) {}
}

class Future {
  static void wait(List<int> values, {void Function(int)? cleanUp}) {}
}

abstract class Parent implements Iterable<int> {
  @override
  int firstWhere(bool Function(int)? test, {int Function()? orElse}) => 0;

  @override
  Iterable<int> where(bool Function(int) test) => throw 0;
}

abstract class Child extends Parent {
  void inherited() {
    super.firstWhere((value) => true, orElse: null); /* expect: null-closures */
  }
}

void callbacks(Child child) {
  Timer.run(null); /* expect: null-closures */
  Future.wait([], cleanUp: null); /* expect: null-closures */
  child.firstWhere(orElse: () => 1, null);
  child.firstWhere((value) => true,
      orElse: /* expect: null-closures */
          null);
  child.firstWhere((value) => true, orElse: (null));
  child.firstWhere((value) => true, orElse: (/* grouped */ (null)));
  Timer.run((null));
  Timer.run(/* bare literal */ null); /* expect: null-closures */
  child.where((value) => true).firstWhere((value) => true, orElse: null); /* expect: null-closures */
}
