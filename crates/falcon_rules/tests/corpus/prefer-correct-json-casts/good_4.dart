import 'dart:convert';

void generic<T>(String text) {
  final typed = jsonDecode(text) as List<T>;
  final mapped = jsonDecode(text) as Map<String, T>;
  final ignored = jsonDecode(text) as List<void>;
}

void patterns(String text) {
  final json = jsonDecode(text);
  switch (json) {
    case Object() as List<Object?>:
      break;
    default:
      break;
  }
  final result = switch (json) {
    Object() as Map<String, Object?> => 1,
    _ => 0,
  };
}
