// `this.` needed because of a shadowing parameter or local, or no `this.`.
class A {
  int x = 0;
  void m(int x) {
    this.x = x;
  }
}

class B {
  int value = 0;
  void update() {
    var value = 5;
    this.value = value;
  }
}

class C {
  int count = 0;
  void inc() {
    count += 1;
  }
  int get doubled => count * 2;
}

class D {
  int total = 0;
  void add(int total) {
    this.total += total;
  }
}

class E {
  void run(int helper) {
    this.helper();
  }
  void helper() {}
}

class ScopedBindings {
  int value = 0;

  void catchesAndLoops() {
    try {
      throw 1;
    } catch (value) {
      print(this.value + value.hashCode);
    }
    for (final value in [1]) {
      print(this.value + value);
    }
    if (Object() case final value) {
      print(this.value + value.hashCode);
    }
  }

  void closures() {
    ((value) => print(this.value + value))(1);
  }
}

class InitializerScope {
  int value = 1;
  void ordinary() {
    final value = this.value;
    print(value);
  }
  void pattern() {
    final (value,) = (this.value,);
    print(value);
  }
  void later() {
    print(this.value);
    final value = 1;
    print(value);
  }
  void capturedLater() {
    void read() => print(this.value);
    final value = 1;
    read();
    print(value);
  }
}

bool isNull(Object? value) => value == null;

extension NullableCheck on Object? {
  bool get isNull => this == null;
}

class ExtensionMember {
  bool check() => this.isNull;
}

class InheritedBase {
  bool get isNull => false;
}

class InheritedMember extends InheritedBase {
  bool check() => this.isNull;
}

class LoopInitializers {
  int x = 0;
  int y = 1;
  List<int> values = [1];
  void run() {
    for (var x = this.x; x < 1; x++) {}
    for (var x = this.y, y = 1; x < y; x++) {}
    for (final values in this.values) {
      print(values);
    }
  }
}

class SwitchGuardBodyScope {
  int y = 0;
  void run(int v) {
    switch (v) {
      // The SDK linter's ancestor scope includes the case body's declarations.
      case _ when this.y > 0:
        print(this.y);
        var y = 1;
        print(y);
    }
  }
}
