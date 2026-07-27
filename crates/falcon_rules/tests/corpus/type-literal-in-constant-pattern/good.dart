import 'missing.dart' as prefix;

class LocalType {}
enum State { ready }

void typePatterns(Object? value) {
  if (value case int _) {}
  if (value case LocalType()) {}
  if (value case const (int)) {}
  if (value case State.ready) {}
  if (value case prefix.RemoteType) {}
}

void shadowed(Object? value, Object? LocalType) {
  if (value case LocalType) {}
  {
    const int = 1;
    if (value case int) {}
  }
}

class MemberShadow {
  final Object? LocalType = null;

  void check(Object? value) {
    if (value case LocalType) {}
  }
}
