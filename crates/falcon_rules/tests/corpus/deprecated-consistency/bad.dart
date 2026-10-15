@deprecated
class A {
  A(); /* expect: deprecated-consistency */
}

@Deprecated('Use B2 instead')
class B {
  B.named(); /* expect: deprecated-consistency */

  @Deprecated('ok')
  B();
}

class C {
  C({this.field}); /* expect: deprecated-consistency */

  @Deprecated('Use x instead')
  Object? field;
}

class D {
  D({@Deprecated('Use y instead') this.field});

  Object? field; /* expect: deprecated-consistency */
}

class E {
  E(this.value, {@deprecated this.legacy});

  final int value;
  final int? legacy; /* expect: deprecated-consistency */
}

@deprecated
enum Mode {
  on(1),
  off(0);

  const Mode(this.code); /* expect: deprecated-consistency */
  final int code;
}
@Deprecated.extend()
class Extendable {
  Extendable(); /* expect: deprecated-consistency */
}

@Deprecated.implement()
class Implementable {
  Implementable(); /* expect: deprecated-consistency */
}
@deprecated
extension type Representation(int value) { /* expect: deprecated-consistency */
  Representation.named(this.value); /* expect: deprecated-consistency */
}

@deprecated
extension type PrivateRepresentation._(int value) {} /* expect: deprecated-consistency */

typedef Dep = Deprecated;

@Dep('old')
class Aliased {
  Aliased(); /* expect: deprecated-consistency */
}

@Dep.extend()
class AliasedNamed {
  AliasedNamed(); /* expect: deprecated-consistency */
}

@deprecated
extension type const ConstRepresentation(int value) {} /* expect: deprecated-consistency */

@deprecated
extension type const ConstNamedRepresentation._(int value) {} /* expect: deprecated-consistency */
