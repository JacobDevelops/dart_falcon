import 'bad_static.dart' as p;

class StaticApi {
  @deprecated
  static void oldMethod() {}
  @deprecated
  static int oldField = 0;
  @deprecated
  static int get oldGetter => 0;
  @deprecated
  static set oldSetter(int value) {}
  static void configure({@deprecated int oldOption = 0}) {}
}
class Base {
  @deprecated
  static void old() {}
}
void use() {
  StaticApi.oldMethod(); /* expect: deprecated-member-use-from-same-package */
  print(StaticApi.oldField); /* expect: deprecated-member-use-from-same-package */
  StaticApi.oldField = 1; /* expect: deprecated-member-use-from-same-package */
  print(StaticApi.oldGetter); /* expect: deprecated-member-use-from-same-package */
  StaticApi.oldSetter = 1; /* expect: deprecated-member-use-from-same-package */
  StaticApi.configure(oldOption: 1); /* expect: deprecated-member-use-from-same-package */
  p.Base.old(); /* expect: deprecated-member-use-from-same-package */
}
