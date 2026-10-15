typedef B = bool;

class LocalEnvironment {
  bool fromEnvironment(String name) => false;
  bool hasEnvironment(String name) => false;
}

bool parameterShadow(LocalEnvironment B) => B.fromEnvironment('a');

bool localShadow() {
  final B = LocalEnvironment();
  return B.hasEnvironment('a');
}
