// Declaration-only SDK excerpt for SDK-free semantic tests.
library dart.convert;

external dynamic jsonDecode(String source, {Object? Function(Object? key, Object? value)? reviver});
external JsonCodec get json;

class JsonCodec {
  external const JsonCodec();
  external dynamic decode(String source, {Object? Function(Object? key, Object? value)? reviver});
}

class JsonDecoder {
  external const JsonDecoder([Object? Function(Object? key, Object? value)? reviver]);
  external dynamic convert(String input);
}
