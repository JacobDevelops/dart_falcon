class Callable {
  const Callable();
  void call() {}
}

void accept(Function callback) {}
void acceptNamed({required Function callback}) {}
void acceptsString(String value) {}

Function returnExplicit() => const Callable().call;
Function topLevelCallback = const Callable().call;

class Receiver {
  Receiver(Function callback, {required Function named});
  Receiver.named({required Function callback});

  Function fieldCallback = const Callable().call;
  static Function staticCallback = const Callable().call;

  void instance(Function callback, {required Function named}) {}
  static void staticMethod(Function callback, {required Function named}) {}

  Function returnMethod() => const Callable().call;

  void explicit(Callable callback, Receiver receiver) {
    Function localCallback = callback.call;
    localCallback = callback.call;
    fieldCallback = callback.call;
    this.fieldCallback = callback.call;
    Receiver.staticCallback = callback.call;
    topLevelCallback = callback.call;

    accept(callback.call);
    acceptNamed(callback: callback.call);
    instance(callback.call, named: callback.call);
    receiver.instance(callback.call, named: callback.call);
    Receiver.staticMethod(callback.call, named: callback.call);
    Receiver(callback.call, named: callback.call);
    new Receiver(callback.call, named: callback.call);
    Receiver.named(callback: callback.call);
  }

  void memberShadowsTopLevel(Callable callback) {
    void accept(String value) {}
    accept(callback);
  }
}

void lexicalShadowing(Callable callback) {
  void accept(String value) {}
  accept(callback);

  void Receiver(String value) {}
  Receiver(callback);

  void localFunction(String value, {required String named}) {}
  localFunction(callback, named: callback);

  void Function(String) functionValue = acceptsString;
  functionValue(callback);

  final inferredFunctionValue = acceptsString;
  inferredFunctionValue(callback);
}

void importedOrUnknownCalls(Callable callback, dynamic importedReceiver) {
  importedAccept(callback);
  importedReceiver.accept(callback);
}

class BaseStringReceiver {
  void accept(Object value) {}
  set callback(Object value) {}
}

class OwnStringReceiver extends BaseStringReceiver {
  @override
  void accept(Object value) {}

  void declaredMembersShadowTopLevel(Callable callable) {
    accept(callable);
  }
}

class ChildStringReceiver extends BaseStringReceiver {
  void inheritedMembersShadowTopLevel(Callable callable) {
    this.accept(callable);
    callback = callable;
    this.callback = callable;
  }
}

void prefixedImportedTypes(
  ext.Callable callback,
  ext.Receiver receiver,
) {
  accept(callback);
  receiver.instance(callback);
  ext.Receiver(callback);
  new ext.Receiver(callback);
}

void reassignedFunctionValue(Callable callback) {
  Function target = accept;
  target = acceptsString;
  target(callback);
}

void scopedShadowing(Callable callback) {
  {
    void accept(String value) {}
    accept(callback);
  }
  accept(callback.call);
}

// A cyclic `extends` chain is invalid Dart but still parses; the lookup must
// terminate instead of walking the cycle forever.
class CyclicA extends CyclicB {}

class CyclicB extends CyclicA {}

void cyclicHierarchy(CyclicA value, Callable callback) {
  value.whatever(callback);
}

class GenericCallable {
  void call<T>(T value) {}
}

void explicitCompositeTearoffs(
  Callable first,
  Callable second,
  Callable? nullable,
  GenericCallable generic,
  bool condition,
) {
  void Function() conditional = condition ? first.call : second.call;
  void Function() coalesced = nullable?.call ?? second.call;
  void Function(int) instantiated = generic.call<int>;
  var spread = <void Function()>[...[first.call]];
  (void Function(), int) record = (first.call, 1);
}
