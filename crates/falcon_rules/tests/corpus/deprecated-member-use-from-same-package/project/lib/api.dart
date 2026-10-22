@deprecated
void oldFunction() { oldFunction(); }
@Deprecated('  Use `freshValue`.  ')
int oldValue = 1;
@Deprecated('.')
class Legacy {}
class Api {
  Api();
  @Deprecated('Use `Api()`.')
  Api.old();
  @deprecated
  void oldMethod() { oldMethod(); }
  @deprecated
  int oldField = 0;
  @Deprecated('Use `freshGetter`.')
  int get oldGetter => 1;
  @deprecated
  set oldSetter(int value) {}
  void configure({@Deprecated('Use fresh') int oldParameter = 0, int fresh = 0}) {}
}
void options({@deprecated int oldOption = 0, int fresh = 0}) {}
void freshFunction() {}
@deprecated
class OldContainer {
  void exempt(Api api) { api.oldMethod(); oldFunction(); }
}
