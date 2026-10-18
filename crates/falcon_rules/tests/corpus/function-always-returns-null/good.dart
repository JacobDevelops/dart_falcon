Object? mixed(bool flag) { if (flag) return null; return 1; }
Object? fallthrough(bool flag) { if (flag) return null; }
Object? empty() {}
void bare() { return; }
void explicitVoid() { return null; }
Object? variable(Object? value) => value;
Object? shadow(Object? empty) { return empty; }
Object? nested(Object? value) {
  Object inner() => 1;
  return value;
}
Object? finallyWins() { try { return null; } finally { return 1; } }
Object? loop(bool flag) { while (flag) { return null; } return null; }
Iterable<Object?> generator() sync* { yield null; }
Future<Object?> asynchronous() async => null;
Object? get emptyGetter => null;
class Example { Object? get value => null; }

typedef VoidCallback = void Function();
void onPressed(VoidCallback callback) {}
void useEffect(Object? Function() callback) {}
void callbacks() {
  VoidCallback callback = () => null;
  onPressed(() { return null; });
  useEffect(() { return null; });
  callback();
}
abstract class Contract {
  Object? result();
  Object? get value;
}
class Implementation implements Contract {
  @override
  Object? result() => null;
  @override
  Object? get value => null;
}
const environmentValue = bool.fromEnvironment('RETURN_NULL') ? null : 1;
Object? environmentDependent() => environmentValue;
Object? capturedConstant() {
  const value = null;
  return value;
}

const shadowedResult = null;
class OwnGetterShadow {
  Object? current;
  Object? get shadowedResult => current;
  Object? result() => shadowedResult;
}
class GetterBase {
  Object? current;
  Object? get shadowedResult => current;
}
class InheritedGetterShadow extends GetterBase {
  Object? result() => shadowedResult;
}

extension NamedGetterShadow on String {
  Object? get shadowedResult => this;
  Object? namedResult() => shadowedResult;
}
extension on List<int> {
  Object? get shadowedResult => this;
  Object? unnamedResult() => shadowedResult;
}
