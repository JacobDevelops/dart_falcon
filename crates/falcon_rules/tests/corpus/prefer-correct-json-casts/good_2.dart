dynamic jsonDecode(String text) => <int>[1];

void custom(String text) {
  final value = jsonDecode(text) as List<int>;
}
