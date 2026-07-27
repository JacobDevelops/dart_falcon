class Callable {
  const Callable();
  void call() {}
}

class ConstructorExpectedTypes {
  final Function callback;
  final Function defaultCallback;

  const ConstructorExpectedTypes(
    Callable callback, {
    this.defaultCallback = const Callable(), /* expect: implicit-call-tearoffs */
  }) : callback = callback; /* expect: implicit-call-tearoffs */
}

void accept(Function callback) {}
void acceptNamed({required Function callback}) {}
Function returnTopLevel() => const Callable(); /* expect: implicit-call-tearoffs */
Function topLevelCallback = const Callable(); /* expect: implicit-call-tearoffs */
set topLevelSetter(Function value) {}

class Receiver {
  Receiver(Function callback, {required Function named});
  Receiver.named({required Function callback});

  Function fieldCallback = const Callable(); /* expect: implicit-call-tearoffs */
  static Function staticCallback = const Callable(); /* expect: implicit-call-tearoffs */
  set callbackSetter(Function value) {}
  static set staticSetter(Function value) {}

  void instance(Function callback, {required Function named}) {}
  static void staticMethod(Function callback, {required Function named}) {}

  Function returnMethod() => const Callable(); /* expect: implicit-call-tearoffs */

  void assignments(Callable callback) {
    Function localCallback = callback; /* expect: implicit-call-tearoffs */
    localCallback = callback; /* expect: implicit-call-tearoffs */
    fieldCallback = callback; /* expect: implicit-call-tearoffs */
    this.fieldCallback = callback; /* expect: implicit-call-tearoffs */
    Receiver.staticCallback = callback; /* expect: implicit-call-tearoffs */
    topLevelCallback = callback; /* expect: implicit-call-tearoffs */
    callbackSetter = callback; /* expect: implicit-call-tearoffs */
    this.callbackSetter = callback; /* expect: implicit-call-tearoffs */
    Receiver.staticSetter = callback; /* expect: implicit-call-tearoffs */
    topLevelSetter = callback; /* expect: implicit-call-tearoffs */
  }

  void methodCalls(Callable callback, Receiver receiver) {
    accept(callback); /* expect: implicit-call-tearoffs */
    acceptNamed(callback: callback); /* expect: implicit-call-tearoffs */
    instance(callback, named: callback.call); /* expect: implicit-call-tearoffs */
    instance(callback.call, named: callback); /* expect: implicit-call-tearoffs */
    this.instance(callback, named: callback.call); /* expect: implicit-call-tearoffs */
    receiver.instance(callback.call, named: callback); /* expect: implicit-call-tearoffs */
    Receiver.staticMethod(callback, named: callback.call); /* expect: implicit-call-tearoffs */
    Receiver.staticMethod(callback.call, named: callback); /* expect: implicit-call-tearoffs */
  }

  void constructorCalls(Callable callback) {
    Receiver(callback, named: callback.call); /* expect: implicit-call-tearoffs */
    Receiver(callback.call, named: callback); /* expect: implicit-call-tearoffs */
    new Receiver(callback, named: callback.call); /* expect: implicit-call-tearoffs */
    Receiver.named(callback: callback); /* expect: implicit-call-tearoffs */
  }

  void localCalls(Callable callback) {
    void localFunction(Function value, {required Function named}) {}
    localFunction(callback, named: callback.call); /* expect: implicit-call-tearoffs */
    localFunction(callback.call, named: callback); /* expect: implicit-call-tearoffs */

    void Function(Function, {required Function named}) functionValue = localFunction;
    functionValue(callback, named: callback.call); /* expect: implicit-call-tearoffs */

    final inferredFunctionValue = localFunction;
    inferredFunctionValue(callback, named: callback.call); /* expect: implicit-call-tearoffs */
  }

  void scoped(List<Callable> callbacks) {
    Function localReturn() => const Callable(); /* expect: implicit-call-tearoffs */
    localReturn();
    for (Callable callback in callbacks) {
      accept(callback); /* expect: implicit-call-tearoffs */
    }
    try {
      throw const Callable();
    } on Callable catch (callback) {
      accept(callback); /* expect: implicit-call-tearoffs */
    }
    final (Callable callback,) = (const Callable(),);
    accept(callback); /* expect: implicit-call-tearoffs */
  }
}

class BaseReceiver {
  void inherited(Function callback) {}
  set inheritedSetter(Function callback) {}
}

class ChildReceiver extends BaseReceiver {
  void inheritedCalls(Callable callback) {
    inherited(callback); /* expect: implicit-call-tearoffs */
    this.inherited(callback); /* expect: implicit-call-tearoffs */
    inheritedSetter = callback; /* expect: implicit-call-tearoffs */
    this.inheritedSetter = callback; /* expect: implicit-call-tearoffs */
  }
}

void topLevelCalls(Callable callback, Receiver receiver) {
  Receiver(callback, named: callback.call); /* expect: implicit-call-tearoffs */
  receiver.instance(callback, named: callback.call); /* expect: implicit-call-tearoffs */
}
