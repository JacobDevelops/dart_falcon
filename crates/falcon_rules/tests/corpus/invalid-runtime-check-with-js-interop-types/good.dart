import 'dart:js_interop';

extension type MyJs(JSObject value) {}

bool sameSdk(JSAny value) => value is JSAny;
bool sdkSubtype(JSArray<JSAny> value) => value is JSObject;
bool typedArraySubtype(JSUint8Array value) => value is JSObject;
bool sameUser(MyJs value) => value is MyJs;
bool nestedSubtype(List<JSArray<JSAny>> value) => value is List<JSObject>;
bool functionParameterSubtype(void Function(JSAny) value) =>
    value is void Function(JSObject);
bool functionReturnSubtype(JSObject Function() value) =>
    value is JSAny Function();
bool ordinary(Object value) => value is String;
JSAny objectAsAny(Object value) => value as JSAny;
JSObject anyAsObject(JSAny value) => value as JSObject;
Object dynamicSource(dynamic value) => value as JSAny;

// The analyzer's `canBeSubtypeOf` never descends into function types.
bool functionParameterWrongDirection(void Function(JSObject) value) =>
    value is void Function(JSAny);
bool functionReturnWrongDirection(JSAny Function() value) =>
    value is JSObject Function();

// Null normalizes to Never, so it does not trigger an interop runtime lint.
bool nullIsInterop() => null is JSAny;
JSAny nullAsInterop() => null as JSAny;

void catchesOrdinary() {
  try {
    throw StateError('ordinary');
  } on List<String> catch (error) {
    error;
  }
}
