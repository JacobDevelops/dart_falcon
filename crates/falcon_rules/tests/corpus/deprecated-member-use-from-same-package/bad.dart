@deprecated
void oldFunction() {}
@Deprecated('Use `newValue`.')
int oldValue = 1;
@Deprecated('Use `NewType`.')
class Legacy {}
class Api {
  Api();
  @deprecated
  Api.old();
  @deprecated
  void oldMethod() {}
}
void use(Api value) {
  oldFunction(); /* expect: deprecated-member-use-from-same-package */
  print(oldValue); /* expect: deprecated-member-use-from-same-package */
  oldValue = 2; /* expect: deprecated-member-use-from-same-package */
  Legacy? variable; /* expect: deprecated-member-use-from-same-package */
  Api.old(); /* expect: deprecated-member-use-from-same-package */
  value.oldMethod(); /* expect: deprecated-member-use-from-same-package */
}

class Properties {
  @deprecated
  int oldField = 0;
  @deprecated
  int get oldGetter => 0;
  @deprecated
  set oldSetter(int value) {}
}
class InheritedProperties extends Properties {}
void propertyUses(InheritedProperties value) {
  print(value.oldField); /* expect: deprecated-member-use-from-same-package */
  print(value.oldGetter); /* expect: deprecated-member-use-from-same-package */
}
class GlobalUses {
  void call() {
    oldFunction(); /* expect: deprecated-member-use-from-same-package */
  }
}
/// [oldFunction] /* expect: deprecated-member-use-from-same-package */
void linkedFunction() {}
/// [Properties.oldGetter] /* expect: deprecated-member-use-from-same-package */
void linkedGetter() {}
class LinkedProperties extends Properties {
  /// [oldField] /* expect: deprecated-member-use-from-same-package */
  void linkedMember() {}
}
enum OldChoice {
  @deprecated
  old,
  current;
}
void patternUse(OldChoice value) {
  switch (value) {
    case OldChoice.old: /* expect: deprecated-member-use-from-same-package */
      break;
    case OldChoice.current:
      break;
  }
}
