import 'dart:convert';
import 'dart:convert' as convert show json, JsonDecoder;

void decode(String text) {
  final objects = jsonDecode(text) as List<Object?>;
  final dynamicObjects = jsonDecode(text) as List<dynamic>;
  final values = jsonDecode(text) as Map<String, Object?>;
  final dynamicValues = jsonDecode(text) as Map<String, dynamic>;
  final broadKeys = jsonDecode(text) as Map<Object, Object?>;
  final nullableBroadKeys = jsonDecode(text) as Map<Object?, Object?>;
  final codec = json.decode(text) as List<Object?>;
  final prefixedCodec = convert.json.decode(text) as Map<String, Object?>;
  final decoder = JsonDecoder().convert(text) as List<Object?>;
  final prefixedDecoder = convert.JsonDecoder().convert(text) as Map<String, Object?>;
  final reviver = JsonDecoder((key, value) => value).convert(text) as List<int>;
}

void shadow(dynamic jsonDecode, dynamic json, dynamic JsonDecoder, dynamic convert, String text) {
  final function = jsonDecode(text) as List<int>;
  final codec = json.decode(text) as List<int>;
  final decoder = JsonDecoder().convert(text) as List<int>;
  final prefixedCodec = convert.json.decode(text) as List<int>;
  final prefixedDecoder = convert.JsonDecoder().convert(text) as List<int>;
}
