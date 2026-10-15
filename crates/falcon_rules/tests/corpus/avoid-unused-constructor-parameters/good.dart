// @dart = 3.13

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

class PrimaryDeclaring(var int value);

class PrimaryFinalDeclaring([final int value = 0]);

class PrimaryInitializing(this.value) {
  final int value;
}

class PrimarySuper(super.x) extends Base;

class PrimaryInitializer(int value) {
  this : stored = value;
  final int stored;
}

class PrimaryAssert({int value = 1}) {
  this : assert(value > 0);
}

class PrimaryBody({int value = 0}) {
  this {
    print(value);
  }
}

class PrimaryField(int value) {
  final int stored = value;
}

class PrimaryFieldClosure(int value) {
  final int Function() callback = () => value;
}

class PrimaryBodyClosure(int value) {
  this {
    void inner() {
      print(value);
    }
    inner();
  }
}

// A body-local shadow must not hide a parameter in a later field initializer.
class PrimarySeparateScopes(int value) {
  this {
    {
      int value = 1;
      print(value);
    }
  }
  final int stored = value;
}

class PrimaryNamedUsed.named(int value) {
  final int stored = value;
}

class PrimaryPrivateUsed._(int value) {
  this : assert(value > 0);
}

class PrimaryDeprecated(@deprecated int old, @Deprecated('use other') int older);

class PrimaryUnderscores(int _, int __);

extension type PrimaryRepresentation(int value) {}

extension type PrimaryPrivateRepresentation._(int value) {}

mixin class PrimaryMixinTrivial();

enum PrimaryEnumField(int value) {
  a(1);
  final int stored = value;
}
