import 'dart:convert';

void patterns(String text) {
  final json = jsonDecode(text);
  switch (json) {
    case Object() as List<int>: /* expect: prefer-correct-json-casts */
      break;
    default:
      break;
  }
  final result = switch (json) {
    Object() as Map<String, int> => 1, /* expect: prefer-correct-json-casts */
    _ => 0,
  };
  final (Object() as List<String>) = json; /* expect: prefer-correct-json-casts */
}
