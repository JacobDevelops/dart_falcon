import 'dart:io';

const bool debug = false;
const int port = 8080;
final String flavor = Platform.environment['FLAVOR'] ?? 'dev';
// A constructor tear-off is not an invocation.
final String Function(String, {String defaultValue}) reader = String.fromEnvironment;
// int.hasEnvironment / String.hasEnvironment do not exist; only bool's is checked.
final parsed = int.parse('42');
final text = String.fromCharCodes([72, 105]);

class Settings {
  const Settings.fromEnvironment();
  static const Settings settings = Settings.fromEnvironment();
}

void main() {
  print([debug, port, flavor, reader, parsed, text, Settings.settings]);
}
