class Foo {
  final Bar bar;
  Foo(this.bar);
  Foo.fromBar(this.bar);
}

class Bar {
  Foo foo() => Foo.fromBar(this); /* expect: use-to-and-as-if-applicable */

  Foo convert() { /* expect: use-to-and-as-if-applicable */
    return Foo(this);
  }

  Foo _wrap() => new Foo(this); /* expect: use-to-and-as-if-applicable */

  Foo tofoo() => Foo(this); /* expect: use-to-and-as-if-applicable */

  Foo assign() => Foo.fromBar(this); /* expect: use-to-and-as-if-applicable */

  void use() => _wrap();
}

extension BarX on Bar {
  Foo view() => Foo(this); /* expect: use-to-and-as-if-applicable */
}

class Negatable {
  Negatable operator ~() => Wrapped(this); /* expect: use-to-and-as-if-applicable */
}
class Wrapped extends Negatable {
  Wrapped(Negatable value);
}
