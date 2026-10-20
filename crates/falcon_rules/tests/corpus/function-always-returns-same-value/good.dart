int mixed(bool flag) { if (flag) return 1; return 2; }
int? fallthrough(bool flag) { if (flag) return 1; }
Object? empty() {}
Object? nullOnly() => null;
int variable(int value) => value;
const answer = 42;
int shadow(int answer) => answer;
int nested(int value) {
  Object? inner() => null;
  return value;
}
int finallyWins() { try { return 1; } finally { return 2; } }
int loop(bool flag) { while (flag) { return 1; } return 1; }
List<int> allocated() => [1, 2];
List<int> allocatedBranches(bool flag) { if (flag) return [1]; return [1]; }
Iterable<int> generator() sync* { yield 1; }
Future<int> asynchronous() async => 1;
int get answerGetter => 42;
class Example { int get value => 42; }

int arrow() => 42;
bool direct() { return true; }
typedef VoidCallback = void Function();
void onPressed(VoidCallback callback) {}
void useEffect(Object? Function() callback) {}
void callbacks() {
  VoidCallback callback = () => true;
  onPressed(() { return true; });
  useEffect(() { return true; });
  callback();
}
abstract class Contract {
  int result(bool flag);
  int get value;
}
class Implementation implements Contract {
  @override
  int result(bool flag) { if (flag) return 42; return 42; }
  @override
  int get value => 42;
}
bool differentEnvironmentFlags(bool branch) {
  if (branch) return const bool.fromEnvironment('FIRST');
  return const bool.fromEnvironment('SECOND');
}
const environmentFlag = bool.fromEnvironment('FIRST');
const wrappedFlag = !environmentFlag;
bool transitiveEnvironment(bool branch) {
  if (branch) return wrappedFlag;
  return true;
}
bool capturedEnvironment(bool branch) {
  const first = bool.fromEnvironment('FIRST');
  const second = bool.fromEnvironment('SECOND');
  if (branch) return first;
  return second;
}

const shadowedResult = 42;
class OwnGetterShadow {
  int current = 0;
  int get shadowedResult => current;
  int result(bool flag) {
    if (flag) return shadowedResult;
    return 42;
  }
}
class GetterBase {
  int current = 0;
  int get shadowedResult => current;
}
class InheritedGetterShadow extends GetterBase {
  int result(bool flag) {
    if (flag) return shadowedResult;
    return 42;
  }
}

extension NamedGetterShadow on String {
  int get shadowedResult => length;
  int namedResult(bool flag) {
    if (flag) return shadowedResult;
    return 42;
  }
}
extension on List<int> {
  int get shadowedResult => length;
  int unnamedResult(bool flag) {
    if (flag) return shadowedResult;
    return 42;
  }
}

bool freshListEquality(bool flag) {
  if (flag) return <int>[] == <int>[];
  return true;
}
bool freshSetEquality(bool flag) {
  if (flag) return <int>{} != <int>{};
  return false;
}
bool freshMapIdentity(bool flag) {
  if (flag) return identical(<int, int>{}, <int, int>{});
  return true;
}
class FreshValue { const FreshValue(); }
bool freshConstructorEquality(bool flag) {
  if (flag) return FreshValue() == FreshValue();
  return true;
}
bool nestedRecordIdentity(bool flag) {
  if (flag) return identical((<int>[],), (<int>[],));
  return true;
}
bool nestedRecordEquality(bool flag) {
  if (flag) return (<int>[],) == (<int>[],);
  return true;
}
