const selected = 2;
void example(int selected) {
  switch (selected) {
    case 2: print('two');
    default: print('other');
  }
  final value = switch (selected) {
    2 => 'two',
    _ => 'other',
  };
  print(value);
}

void runtimeAndPatterns(Object value, List<int> values) {
  for (final selected in values) {
    switch (selected) {
      case 2: break;
      default: break;
    }
  }
  if (value case int selected) {
    final result = switch (selected) { 2 => true, _ => false };
    print(result);
  }
  final result = [
    for (final selected in values)
      switch (selected) { 2 => true, _ => false },
  ];
  const local = 2;
  // Captured locals have no environment-dependency provenance.
  switch (local) {
    case 2: break;
    default: break;
  }
  switch (const int.fromEnvironment('NUMBER')) {
    case 2: break;
    default: break;
  }
  print(result);
}
const environment = String.fromEnvironment('MODE');
const derived = environment == 'debug';
void environmentSwitch() {
  switch (derived) {
    case true: break;
    case false: break;
  }
}

const shadowedGetter = 1;
class GetterShadow {
  int get shadowedGetter => DateTime.now().millisecondsSinceEpoch;
  void check() {
    switch (shadowedGetter) {
      case 1: break;
      default: break;
    }
  }
}
class InheritedGetterShadow extends GetterShadow {
  void inheritedCheck() {
    switch (shadowedGetter) {
      case 1: break;
      default: break;
    }
  }
}

void shadowedMethod() {}
class MethodShadow {
  void shadowedMethod() {}
  void check() {
    switch (shadowedMethod) {
      default: break;
    }
  }
}

const extensionFlag = true;
extension on String {
  bool get extensionFlag => isEmpty;
  void checkUnnamedExtension() {
    switch (extensionFlag) {
      case true: break;
      case false: break;
    }
  }
}
extension NamedFlag on List<int> {
  bool get extensionFlag => isEmpty;
  void checkNamedExtension() {
    switch (extensionFlag) {
      case true: break;
      case false: break;
    }
  }
}
