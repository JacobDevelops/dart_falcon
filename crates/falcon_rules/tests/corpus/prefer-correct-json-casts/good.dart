import 'dart:convert';

void decode(String text, dynamic unrelated) {
  final json = jsonDecode(text) as Map<String, Object?>;
  final items = (json['items'] as List<Object?>).cast<int>().toList();
  final map = (json['map'] as Map<String, dynamic>).cast<String, int>();
  final list = jsonDecode(text) as List<dynamic>;
  final raw = jsonDecode(text) as Map;
  final scalar = json['count'] as int;
  final nonJson = unrelated as List<int>;
  final converted = items as List<int>;
  {
    final json = <String, dynamic>{'items': <int>[1]};
    final typed = json['items'] as List<int>;
  }
  final revived = jsonDecode(text, reviver: (key, value) => value) as List<int>;
}
