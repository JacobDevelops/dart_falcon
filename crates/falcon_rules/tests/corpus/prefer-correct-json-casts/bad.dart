import 'dart:convert';
import 'dart:convert' as convert;

void decode(String text) {
  final numbers = jsonDecode(text) as List<int>; /* expect: prefer-correct-json-casts */
  final mapping = convert.jsonDecode(text) as Map<String, int>; /* expect: prefer-correct-json-casts */
  final json = jsonDecode(text) as Map<String, Object?>;
  final items = json['items'] as List<String>; /* expect: prefer-correct-json-casts */
  final alias = json;
  final nested = alias['nested'] as Map<String, List<int>>; /* expect: prefer-correct-json-casts */
  final wrongKeys = jsonDecode(text) as Map<int, dynamic>; /* expect: prefer-correct-json-casts */
  if (json['items'] case Object() as List<int>) {} /* expect: prefer-correct-json-casts */
}
