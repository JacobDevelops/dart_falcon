@deprecated
class A {
  @deprecated
  A();
}

@Deprecated('Use B2 instead')
class B {
  @Deprecated('Use B2.named instead')
  B.named();
}

class C {
  C({@Deprecated('Use x instead') this.field});

  @Deprecated('Use x instead')
  Object? field;
}

// Neither side deprecated.
class D {
  D(this.field);

  final Object? field;
}

// Private fields are not part of the public API.
class E {
  E(this._secret);

  @deprecated
  final int _secret;

  int get secret => _secret;
}

// A deprecated constructor in a non-deprecated class is fine.
class F {
  @Deprecated('Use F.create')
  F();

  F.create();
}

// Static fields are not initialized by formals.
class G {
  @deprecated
  static int counter = 0;

  G(int counter);
}

@Deprecated.extend()
class ConsistentNamed {
  @deprecated
  ConsistentNamed();
}

class ConsistentField {
  ConsistentField(@deprecated this.value);
  @Deprecated('old')
  final int value;
}

@Deprecated.new('old')
class ExplicitUnnamed {
  @Deprecated.new('old')
  ExplicitUnnamed();
}

// Representation fields have no reportable field-fragment name offset.
extension type DeprecatedRepresentationParameter(int value) {
  DeprecatedRepresentationParameter.named(@deprecated this.value);
}

typedef Dep = Deprecated;

@Dep('old')
class ConsistentAlias {
  @Dep('old')
  ConsistentAlias();
}
