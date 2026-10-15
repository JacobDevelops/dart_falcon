// @dart = 3.11
extension type FirstView(Before a) implements Before {
  FirstView.checked(this.a) {
    assert(first > 0); /* expect: prefer-asserts-in-initializer-lists */
  }
}

class Before {
  int get first => 0;
}

extension type BeforeView(Before a) implements Before {
  BeforeView.checked(this.a) {
    assert(first > 0);
  }
}

class Unrelated {
  int get second => 0;
}

extension type AfterView(Before a) implements Before {
  AfterView.checked(this.a) {
    assert(first > 0); /* expect: prefer-asserts-in-initializer-lists */
  }
}

extension type SameChain(Unrelated a) implements Unrelated {
  SameChain.checked(this.a) {
    assert(second > 0);
  }
}

class Derived extends Before {}

extension type InheritedView(Before a) implements Before {
  InheritedView.checked(this.a) {
    assert(first > 0);
  }
}
