class Parent {
  @deprecated
  Parent();
  @deprecated
  Parent.old({@deprecated int option = 0});
}
// No constructor declaration, so no implicit super-constructor use is reported.
class Implicit extends Parent {}
class Child extends Parent {
  Child() : super(); /* expect: deprecated-member-use-from-same-package */
  Child.named() : super.old( /* expect: deprecated-member-use-from-same-package */
    option: 1, /* expect: deprecated-member-use-from-same-package */
  );
}
class Redirect implements Factory {
  @deprecated
  Redirect.old();
  Redirect() : this.old(); /* expect: deprecated-member-use-from-same-package */
}
class Factory {
  factory Factory() = Redirect.old; /* expect: deprecated-member-use-from-same-package */
}
@deprecated
class OldMarker { const OldMarker(); }
@OldMarker() /* expect: deprecated-member-use-from-same-package */
void annotated() {}
const message = 'Use fresh';
@Deprecated(message)
void constantMessage() {}
@deprecated
typedef OldAlias = int;
void use() {
  constantMessage(); /* expect: deprecated-member-use-from-same-package */
  OldAlias value = 0; /* expect: deprecated-member-use-from-same-package */
}

class Legacy<T> {
  @Deprecated('Use modern')
  const Legacy.fromString(String value);
  const Legacy.adaptive({@Deprecated('Use modern') int? groupValue});
}
void constructLegacy() {
  const Legacy.fromString('value'); /* expect: deprecated-member-use-from-same-package */
  const Legacy<int>.fromString('value'); /* expect: deprecated-member-use-from-same-package */
  new Legacy.fromString('value'); /* expect: deprecated-member-use-from-same-package */
  Legacy<int>.fromString('value'); /* expect: deprecated-member-use-from-same-package */
  Legacy<int>.adaptive(groupValue: 1); /* expect: deprecated-member-use-from-same-package */
  const Legacy.adaptive(groupValue: 1); /* expect: deprecated-member-use-from-same-package */
  const Legacy<int>.adaptive(groupValue: 1); /* expect: deprecated-member-use-from-same-package */
}

typedef LegacyAlias<T> = Legacy<T>;
void constructAlias() {
  const LegacyAlias.fromString('value'); /* expect: deprecated-member-use-from-same-package */
  const LegacyAlias<int>.fromString('value'); /* expect: deprecated-member-use-from-same-package */
  new LegacyAlias.fromString('value'); /* expect: deprecated-member-use-from-same-package */
  LegacyAlias<int>.fromString('value'); /* expect: deprecated-member-use-from-same-package */
  const LegacyAlias.adaptive(groupValue: 1); /* expect: deprecated-member-use-from-same-package */
  LegacyAlias<int>.adaptive(groupValue: 1); /* expect: deprecated-member-use-from-same-package */
}

class AliasTarget {
  @deprecated
  const AliasTarget();
  const AliasTarget.options([@deprecated int option = 0]);
}
typedef TargetAlias = AliasTarget;
void constructUnnamedAlias() {
  TargetAlias(); /* expect: deprecated-member-use-from-same-package */
  const TargetAlias(); /* expect: deprecated-member-use-from-same-package */
  new TargetAlias(); /* expect: deprecated-member-use-from-same-package */
  TargetAlias.options(1); /* expect: deprecated-member-use-from-same-package */
  const TargetAlias.options(1); /* expect: deprecated-member-use-from-same-package */
}
