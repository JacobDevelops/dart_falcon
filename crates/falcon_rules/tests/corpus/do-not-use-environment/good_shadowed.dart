// A file-level declaration named like a core type shadows dart:core.
class String {
  const String.fromEnvironment(Object name);
}

const value = String.fromEnvironment('VALUE');

void main() {
  print(value);
}
