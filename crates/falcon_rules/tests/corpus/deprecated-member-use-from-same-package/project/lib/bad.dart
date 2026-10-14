import 'api.dart';
import 'api.dart' as api;
void uses(Api value) {
  oldFunction(); /* expect: deprecated-member-use-from-same-package */
  final tearoff = oldFunction; /* expect: deprecated-member-use-from-same-package */
  print(oldValue); /* expect: deprecated-member-use-from-same-package */
  oldValue = 2; /* expect: deprecated-member-use-from-same-package */
  value.oldMethod(); /* expect: deprecated-member-use-from-same-package */
  print(value.oldField); /* expect: deprecated-member-use-from-same-package */
  print(value.oldGetter); /* expect: deprecated-member-use-from-same-package */
  value.oldSetter = 3; /* expect: deprecated-member-use-from-same-package */
  Api.old(); /* expect: deprecated-member-use-from-same-package */
  value.configure(oldParameter: 1); /* expect: deprecated-member-use-from-same-package */
  options(oldOption: 2); /* expect: deprecated-member-use-from-same-package */
  api.oldFunction(); /* expect: deprecated-member-use-from-same-package */
  Legacy? legacy; /* expect: deprecated-member-use-from-same-package */
  final type = Legacy; /* expect: deprecated-member-use-from-same-package */
  print(tearoff);
}
