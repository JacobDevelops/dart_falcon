import 'dart:convert';
import 'dart:convert' as convert show json, JsonDecoder;

void decode(String text) {
  final objects = jsonDecode('[1]') as List<Object>; /* expect: prefer-correct-json-casts */
  final values = jsonDecode('{"value":1}') as Map<String, Object>; /* expect: prefer-correct-json-casts */
  final prefixedObjects = convert.json.decode(text) as List<Object>; /* expect: prefer-correct-json-casts */
  final codec = json.decode(text) as List<int>; /* expect: prefer-correct-json-casts */
  final decoder = JsonDecoder().convert(text) as List<int>; /* expect: prefer-correct-json-casts */
  final prefixedDecoder = convert.JsonDecoder().convert(text) as List<int>; /* expect: prefer-correct-json-casts */
  final explicitDecoder = new JsonDecoder().convert(text) as List<int>; /* expect: prefer-correct-json-casts */
  final explicitPrefixedDecoder = new convert.JsonDecoder().convert(text) as Map<String, Object>; /* expect: prefer-correct-json-casts */
}
