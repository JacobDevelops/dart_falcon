class Good {
  final int a;

  Good(this.a);
}

class UsedInInitializer {
  final int value;

  UsedInInitializer(int input) : value = input * 2;
}

class UsedInBody {
  int total = 0;

  UsedInBody(int first) {
    total = first;
  }
}

class UsedInAssert {
  UsedInAssert(int n) : assert(n > 0);
}

class Base {
  Base(this.x);
  final int x;
}

class UsedInSuper extends Base {
  UsedInSuper(int x) : super(x + 1);
}

class SuperParameter extends Base {
  SuperParameter(super.x);
}

class Deprecations {
  Deprecations(@deprecated int old, @Deprecated('use other') int older);
}

class Underscores {
  Underscores(int _, int __);
}

class Redirecting {
  Redirecting.make();
  factory Redirecting(int ignored) = Redirecting.make;
}

class External {
  external External(int value);
}

class UsedInClosure {
  void Function() callback = () {};

  UsedInClosure(int value) {
    callback = () => print(value);
  }
}

class Redirect {
  final int value;

  Redirect(this.value);
  Redirect.twice(int value) : this(value * 2);
}
