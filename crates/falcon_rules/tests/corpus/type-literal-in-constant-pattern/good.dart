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

// Matching Type values against type literals is intentional.
void typeValues(Type value) {
  if (value case int) {}
  switch (value) {
    case String:
      break;
    default:
      break;
  }
}

void boundedTypeValues<T extends Type>(T value) {
  if (value case int) {}
}

void dynamicTypedPatterns(Object? value) {
  if (value case dynamic _) {}
  if (value case dynamic item) {
    print(item);
  }
  if (value case const (dynamic)) {}
}

void dynamicTypeValues(Type value) {
  if (value case dynamic) {}
  switch (value) {
    case dynamic:
      break;
    default:
      break;
  }
}
