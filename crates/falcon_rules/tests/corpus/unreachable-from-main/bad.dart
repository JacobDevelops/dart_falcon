void main() {
  Used();
  print(Used.new);
  Child();
  Holder().value;
  print(Shade.dark);
  helper();
  var shadowed = 1;
  print(shadowed);
  Object probe = 1;
  if (probe is OnlyInIsCheck) {}
  if (probe case const OnlyInConstantPattern()) {}
  Shadower().run();
  if (probe case OnlyInObjectPattern()) {}
  dynamic untyped = probe;
  untyped.sharedName();
}

void helper() {}

class NeverNamed { /* expect: unreachable-from-main */
  void sharedName() {}
}

class Shadower {
  void run() {
    print(label());
  }

  String label() => 'member';
}

String label() => 'top-level'; /* expect: unreachable-from-main */

void orphanFunction() {} /* expect: unreachable-from-main */

int orphanVariable = 1; /* expect: unreachable-from-main */

int shadowed = 2; /* expect: unreachable-from-main */

int get orphanGetter => 7; /* expect: unreachable-from-main */

set orphanSetter(int value) {} /* expect: unreachable-from-main */

typedef OrphanAlias = String; /* expect: unreachable-from-main */

class OrphanClass { /* expect: unreachable-from-main */
  void memberOfUnreachableClass() {}
}

class OrphanWithField { /* expect: unreachable-from-main */
  int orphanField = 0; /* expect: unreachable-from-main */
  static const orphanConstant = 1; /* expect: unreachable-from-main */
}

mixin OrphanMixin {} /* expect: unreachable-from-main */

enum OrphanEnum { a, b } /* expect: unreachable-from-main */

extension OrphanExtension on int {} /* expect: unreachable-from-main */

extension type OrphanExtensionType(int value) {} /* expect: unreachable-from-main */

@pragma('other')
void unrelatedPragma() {} /* expect: unreachable-from-main */

void setUpClass() {} /* expect: unreachable-from-main */

class Used {
  Used();
  Used.named(); /* expect: unreachable-from-main */

  void unusedMethod() {} /* expect: unreachable-from-main */
  int? unusedField; /* expect: unreachable-from-main */
  static void unusedStatic() {} /* expect: unreachable-from-main */
  int get unusedGetter => 0; /* expect: unreachable-from-main */
  set unusedSetter(int value) {} /* expect: unreachable-from-main */
  Used operator +(Used other) => this; /* expect: unreachable-from-main */
}

class Base {
  void overridden() {} /* expect: unreachable-from-main */
}

class Child extends Base {
  @override
  void overridden() {}
}

class OnlyInTypeAnnotation {} /* expect: unreachable-from-main */

class Holder {
  OnlyInTypeAnnotation? value;
}

class OnlyInIsCheck {} /* expect: unreachable-from-main */

class OnlyInConstantPattern { /* expect: unreachable-from-main */
  const OnlyInConstantPattern();
}

class OnlyInObjectPattern {} /* expect: unreachable-from-main */

enum Shade {
  dark,
  light;

  static void unusedEnumStatic() {} /* expect: unreachable-from-main */
}

extension IntExtension on int { /* expect: unreachable-from-main */
  void unusedExtensionMethod() {}
}
