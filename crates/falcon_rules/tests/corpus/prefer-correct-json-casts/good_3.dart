import 'dart:convert';

void shadow(String text, dynamic Function(String) jsonDecode) {
  final value = jsonDecode(text) as List<int>;
}

class Decoder {
  dynamic jsonDecode(String text) => <int>[1];
  void decode(String text) {
    final value = jsonDecode(text) as List<int>;
  }
}

class StaticDecoder {
  static dynamic jsonDecode(String text) => <int>[1];
  static void decode(String text) {
    final value = jsonDecode(text) as List<int>;
  }
}

void safeCodecCalls(String text, dynamic json) {
  final custom = json.decode(text) as List<int>;
  final decoded = JsonDecoder().convert(text) as List<Object?>;
  final revived = JsonDecoder((key, value) => <int>[1]).convert(text) as List<int>;
}

void safeJsonCodec(String text) {
  final decoded = json.decode(text) as Map<String, Object?>;
  final revived = json.decode(text, reviver: (key, value) => <int>[1]) as List<int>;
}

JsonDecoder customDecoder() => JsonDecoder((key, value) => <int>[1]);

void indirectDecoder(String text) {
  final value = customDecoder().convert(text) as List<int>;
}

void unrelatedContainers(String text, dynamic container, dynamic Function(dynamic) transform) {
  final value = container[jsonDecode(text)] as List<int>;
  final converted = transform(jsonDecode(text)) as Map<String, int>;
}
