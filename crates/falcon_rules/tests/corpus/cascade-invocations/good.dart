class Point {
  int x = 0;
  void reset() {}
  static void create() {}
  static void destroy() {}
}

void cascaded() {
  final list = <int>[]
    ..add(1)
    ..add(2);
  print(list);
}

void different(List<int> a, List<int> b) {
  a.add(1);
  b.add(2);
}

void nullAware(List<int>? list) {
  list?.add(1);
  list?.add(2);
}

void staticCalls() {
  Point.create();
  Point.destroy();
}

void dependency(List<int> list) {
  list.add(1);
  list.add(list.length);
}

void awaited() async {
  final values = await Future.value(<int>[]);
  values.add(1);
}

void constDeclaration() {
  const list = <int>[];
  list.length;
}

void ifNull(List<int>? list) {
  list ??= [];
  list.add(1);
}

void single(List<int> list) {
  list.add(1);
}

void separated(List<int> list) {
  list.add(1);
  print('x');
  list.add(2);
}

void chained(Map<String, List<int>> map) {
  map['a']!.add(1);
  map['a']!.add(2);
}

class Listener {
  void addListener(void Function() f) {}
  void removeListener(void Function() f) {}
  void layout() {}
  int width = 0;
}

class Owner<T> {
  final Listener first = Listener();
  final Listener second = Listener();
  final List<T> generic = [];
  void _onChange() {}

  // A tear-off of the field's own class may observe the field.
  void listen() {
    first.addListener(_onChange);
    first.removeListener(_onChange);
  }

  // So may another field of the same class.
  void fields() {
    first.layout();
    first.addListener(second.layout);
  }

  // Property sets through a field never receive a cascade.
  void sets() {
    first.width = 1;
    first.layout();
  }

  Listener third = Listener();

  // Assigning a field writes its setter, a different element than the getter.
  void assigns() {
    third = Listener();
    third.layout();
  }

  // A field typed with the class's type parameters is a fresh member per use.
  void typed(T value) {
    generic.add(value);
    generic.add(value);
  }
}

class Callbacks {
  final void Function() callback = () {};
}

// Calling a function-typed field is no method invocation.
void invokeTwice(Callbacks callbacks) {
  callbacks.callback();
  callbacks.callback();
}

// Dynamic calls have no resolved method element.
void dynamicMethods(dynamic receiver) {
  receiver.first();
  receiver.second();
}

void functionCalls(Function callback) {
  callback.call();
  callback.call();
}

void typedFunctionCalls(void Function() callback) {
  callback.call();
  callback.call();
}

void parenthesizedTargets(Point point) {
  (point).reset();
  (point).reset();
}

class ScopedReceiver {
  void initialize(void Function(ScopedReceiver) callback) {}
  void reset() {}
}

// A captured reference to the outer receiver still breaks the cascade.
void capturedCallback(ScopedReceiver receiver) {
  receiver.initialize((other) {
    receiver.reset();
  });
  receiver.reset();
}
