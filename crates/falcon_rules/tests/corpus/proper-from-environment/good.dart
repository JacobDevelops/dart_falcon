const apiUrl = String.fromEnvironment('API_URL');

const debug = bool.fromEnvironment('DEBUG');

const port = int.fromEnvironment('PORT');

class C {
  static const key = String.fromEnvironment('KEY');
}

const list = [
  String.fromEnvironment('A'),
  String.fromEnvironment('B'),
];

String get env => const String.fromEnvironment('ENV');
