class Foo {
  final Bar bar;
  Foo(this.bar);
  Foo.fromBar(this.bar);
  static Foo make(Bar bar) => Foo(bar);
}

class Bar {
  Foo toFoo() => Foo.fromBar(this);
  Foo asFoo() => Foo(this);
  Foo _toFoo() => Foo(this);
  Foo _asFoo() => Foo(this);

  // Takes a parameter.
  Foo combine(Bar other) => Foo(other);

  // Getters are not checked.
  Foo get foo => Foo(this);

  // Static method call, not a constructor.
  Foo make() => Foo.make(this);

  // More than the receiver is passed.
  Pair pair() => Pair(this, 1);

  // More than one statement.
  Foo build() {
    print('building');
    return Foo(this);
  }
}

class Pair {
  Pair(Bar bar, int n);
}

// Overrides an inherited method.
abstract class Convertible {
  Foo convert();
}

class Baz extends Bar implements Convertible {
  @override
  Foo convert() => Foo(this);
}
