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
