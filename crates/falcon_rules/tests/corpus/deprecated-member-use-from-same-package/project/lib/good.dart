import 'api.dart';
void shadow(void Function() oldFunction, int oldValue) {
  oldFunction();
  print(oldValue);
}
void localShadow() {
  void oldFunction() {}
  oldFunction();
}
void unknown(dynamic value) { value.oldMethod(); value.oldSetter = 1; }
void fresh(Api value) { freshFunction(); value.configure(fresh: 1); options(fresh: 1); Api(); }
@deprecated
void deprecatedUser(Api value) { oldFunction(); value.oldMethod(); Api.old(); }
@Deprecated('Use something else')
Legacy? deprecatedTypeUser() => null;
class FreshOverride extends Api {
  @override
  void oldMethod() {}
}
void overrideUse(FreshOverride value) { value.oldMethod(); }
