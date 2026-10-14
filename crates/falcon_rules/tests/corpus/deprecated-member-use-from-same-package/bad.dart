@deprecated
void oldFunction() {}
@Deprecated('Use newValue')
int oldValue = 1;
@Deprecated('Use NewType!')
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
