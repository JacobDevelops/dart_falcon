import 'dart:js_interop';
import 'dart:core';
import 'dart:core' as core;

extension type MyJs(JSObject value) {}
extension type OtherJs(JSObject value) {}

@JS()
@staticInterop
class StaticJs {}

bool dartAgainstJs(Object value) => value is JSObject; /* expect: invalid-runtime-check-with-js-interop-types */
bool jsAgainstDart(JSAny value) => value is String; /* expect: invalid-runtime-check-with-js-interop-types */
bool unrelatedUsers(MyJs value) => value is OtherJs; /* expect: invalid-runtime-check-with-js-interop-types */
String invalidCast(JSAny value) => value as String; /* expect: invalid-runtime-check-with-js-interop-types */
bool anyIsObject(JSAny value) => value is JSObject; /* expect: invalid-runtime-check-with-js-interop-types */
bool nestedGeneric(List<JSAny> value) => value is List<JSObject>; /* expect: invalid-runtime-check-with-js-interop-types */
bool nestedRecord((JSAny,) value) => value is (JSObject,); /* expect: invalid-runtime-check-with-js-interop-types */

void catchesInterop() {
  try {
    throw StateError('x');
  } on JSAny catch (error) { /* expect: invalid-runtime-check-with-js-interop-types */
    error;
  }
}

bool dynamicIsInterop(dynamic value) => value is JSAny; /* expect: invalid-runtime-check-with-js-interop-types */

// Explicit core prefixes exercise SDK-free imported identity resolution too.
void catchesNestedInterop<T extends JSNumber>() {
  try {
    throw StateError('nested');
  } on core.List<JSString> catch (error) { /* expect: invalid-runtime-check-with-js-interop-types */
    error;
  } on core.List<T> catch (error) { /* expect: invalid-runtime-check-with-js-interop-types */
    error;
  }
}
