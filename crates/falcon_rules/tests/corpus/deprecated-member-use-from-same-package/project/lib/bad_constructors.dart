class Parent {
  @deprecated
  Parent();
  @deprecated
  Parent.old({@deprecated int option = 0});
}
class Implicit extends Parent {} /* expect: deprecated-member-use-from-same-package */
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
