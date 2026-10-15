// @dart = 3.11
class First {
  int get value => 1;
}

// The retained class has the same member name, but a different element.
extension type Own(int a) {
  int get value => a;
  Own.checked(this.a) {
    assert(value > 0); /* expect: prefer-asserts-in-initializer-lists */
  }
}

class Other {
  int get value => 2;
}

extension type OtherView(First a) implements First {
  OtherView.checked(this.a) {
    assert(value > 0); /* expect: prefer-asserts-in-initializer-lists */
  }
}
