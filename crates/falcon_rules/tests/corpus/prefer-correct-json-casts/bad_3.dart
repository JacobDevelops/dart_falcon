import 'dart:convert';
import 'dart:convert' as convert;

class Decoder {
  dynamic jsonDecode(String text) => <int>[1];
}

class ChildDecoder extends Decoder {
  void decodeAgain(String text) {
    final value = jsonDecode(text) as List<int>; /* expect: prefer-correct-json-casts */
  }
}

class Converter {
  dynamic convert(String text) => <int>[1];
}

class ChildConverter extends Converter {
  void decode(String text) {
    final value = convert.jsonDecode(text) as List<int>; /* expect: prefer-correct-json-casts */
  }
}

void codecCalls(String text) {
  final first = json.decode(text) as List<int>; /* expect: prefer-correct-json-casts */
  final second = convert.json.decode(text) as Map<String, int>; /* expect: prefer-correct-json-casts */
  final third = JsonDecoder().convert(text) as List<int>; /* expect: prefer-correct-json-casts */
  final fourth = convert.JsonDecoder().convert(text) as Map<String, int>; /* expect: prefer-correct-json-casts */
  final fifth = new JsonDecoder().convert(text) as List<String>; /* expect: prefer-correct-json-casts */
}
