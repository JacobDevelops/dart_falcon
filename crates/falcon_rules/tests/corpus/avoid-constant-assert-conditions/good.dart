const enabled = true;
void example(bool enabled) {
  assert(enabled);
  assert(!enabled, 'runtime');
}
class Example {
  const Example(int value) : assert(value > 0);
}

void runtimeAndPatterns(bool enabled, Object value, List<bool> flags) {
  assert(enabled == true);
  assert(enabled != false);
  for (final enabled in flags) {
    assert(enabled);
  }
  if (value case bool enabled) {
    assert(enabled);
  }
  const local = true;
  // Captured locals have no environment-dependency provenance.
  assert(local);
  assert(const bool.fromEnvironment('FLAG'));
}
const environment = bool.hasEnvironment('FLAG');
const derived = !environment;
void environmentAssert() {
  assert(derived);
}
class InitializerShadow {
  const InitializerShadow(bool enabled) : assert(enabled);
}

const shadowedGetter = true;
class GetterShadow {
  bool get shadowedGetter => DateTime.now().millisecondsSinceEpoch.isEven;
  void check() {
    assert(shadowedGetter);
  }
}
class InheritedGetterShadow extends GetterShadow {
  void inheritedCheck() {
    assert(shadowedGetter);
  }
}

void shadowedMethod() {}
class MethodShadow {
  void shadowedMethod() {}
  void check() {
    assert(shadowedMethod == shadowedMethod);
  }
}

const extensionFlag = true;
extension on String {
  bool get extensionFlag => isEmpty;
  void checkUnnamedExtension() {
    assert(extensionFlag);
  }
}
extension NamedFlag on List<int> {
  bool get extensionFlag => isEmpty;
  void checkNamedExtension() {
    assert(extensionFlag);
  }
}
