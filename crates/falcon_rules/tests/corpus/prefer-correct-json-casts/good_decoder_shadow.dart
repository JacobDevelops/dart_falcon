import 'dart:convert' hide JsonDecoder;

class JsonDecoder {
  dynamic convert(String text) => <int>[1];
}

void decode(String text) {
  final value = JsonDecoder().convert(text) as List<int>;
  final decoded = json.decode(text) as List<Object?>;
}
