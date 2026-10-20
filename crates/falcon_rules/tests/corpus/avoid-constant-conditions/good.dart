const enabled = true;
void example(bool enabled, int x) {
  if (enabled) {}
  while (x > 0) { x--; }
  do { x++; } while (enabled);
  for (; x < 10; x++) {}
  final result = enabled ? 1 : x;
  final list = [if (enabled) 1];
  final map = {if (enabled) 1: 2};
  print([result, list, map]);
}

const flag = true;
void shadowedPatterns(Object value) {
  if (value case bool flag when flag == true) {}
  switch (value) {
    case bool flag when flag == true: print(flag);
  }
  final values = [for (final flag in <bool>[true, false]) if (flag == true) 1];
  final maps = {for (final flag in <bool>[true, false]) if (flag == true) 1: 2};
  print([values, maps]);
}

void literalAndLocal() {
  if (true) {}
  while (true) { break; }
  const local = true;
  // Captured locals have no environment-dependency provenance.
  if (local == true) {}
  if (const bool.fromEnvironment('FLAG') == true) {}
}
const environment = bool.fromEnvironment('FLAG');
const derived = !environment;
void environmentCondition() {
  if (derived == true) {}
}

const shadowedGetter = true;
class GetterShadow {
  bool get shadowedGetter => DateTime.now().millisecondsSinceEpoch.isEven;
  void check() {
    if (shadowedGetter == true) {}
  }
}
class InheritedGetterShadow extends GetterShadow {
  void inheritedCheck() {
    if (shadowedGetter == true) {}
  }
}

void shadowedMethod() {}
class MethodShadow {
  void shadowedMethod() {}
  void check() {
    if (shadowedMethod == shadowedMethod) {}
  }
}

const extensionFlag = true;
extension on String {
  bool get extensionFlag => isEmpty;
  void checkUnnamedExtension() {
    if (extensionFlag == true) {}
  }
}
extension NamedFlag on List<int> {
  bool get extensionFlag => isEmpty;
  void checkNamedExtension() {
    if (extensionFlag == true) {}
  }
}

class FreshValue { const FreshValue(); }
void freshAllocationConditions() {
  if (<int>[] == <int>[]) {}
  if (<int>{} != <int>{}) {}
  if (identical(<int, int>{}, <int, int>{}) == true) {}
  if (FreshValue() == FreshValue()) {}
  if (identical((<int>[],), (<int>[],)) == true) {}
}
void nestedRecordCondition() {
  if ((<int>[],) == (<int>[],)) {}
}
