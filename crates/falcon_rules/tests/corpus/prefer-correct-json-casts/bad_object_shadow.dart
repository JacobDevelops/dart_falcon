import 'dart:convert';

class Object {}

void decode(String text) {
  final objects = jsonDecode(text) as List<Object>; /* expect: prefer-correct-json-casts */
  final nullableObjects = jsonDecode(text) as List<Object?>; /* expect: prefer-correct-json-casts */
  final values = jsonDecode(text) as Map<String, Object>; /* expect: prefer-correct-json-casts */
  final nullableValues = jsonDecode(text) as Map<String, Object?>; /* expect: prefer-correct-json-casts */
}
