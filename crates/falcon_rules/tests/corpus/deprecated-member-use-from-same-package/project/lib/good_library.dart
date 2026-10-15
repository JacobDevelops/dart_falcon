@Deprecated('This library intentionally exercises deprecated APIs')
library;

import 'api.dart';

/// References [oldFunction] and [Api.oldGetter] are exempt in this library.
void exercise(Api api) {
  oldFunction();
  print(api.oldGetter);
  api.oldSetter = 1;
}
