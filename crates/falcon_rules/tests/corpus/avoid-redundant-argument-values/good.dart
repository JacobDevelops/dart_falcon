void configure({bool verbose = false, int retries = 3, String? name}) {}

void positional(int a, [int b = 1, int c = 2]) {}

void required({required int count}) {}

void numeric({num value = 1}) {}

void listDefault({List<int> values = const []}) {}

class Widget {
  Widget({this.size = 1.0});
  factory Widget.redirect({double size}) = Widget.other;
  Widget.other({this.size = 2.0});
  final double size;
}

void main(bool flag, int n) {
  configure(verbose: true);
  configure(retries: 4);
  configure(name: 'n');
  configure(verbose: flag);
  positional(0, 1, 3);
  positional(0, 1, 2 + 1);
  required(count: 0);
  numeric(value: 1.0);
  listDefault(values: []);
  configure(retries: n);
  Widget(size: 2.0);
  Widget.redirect(size: 1.0);
  print(1);
}

void shadowed(void Function({bool verbose}) configure) {
  configure(verbose: false);
}

class Box<E> {
  void fill(E value, [int start = 0]) {}
}

void generic(Box<int> box, List<int> list) {
  box.fill(1, 0);
  list.setRange(0, 1, list, 0);
}

const lexicalDefault = 2;
void capturedDefaultDoesNotUseTopLevel() {
  const lexicalDefault = 1;
  void local([int value = lexicalDefault]) {}
  final alias = local;
  local(2);
  alias(2);
  {
    const lexicalDefault = 3;
    local(3);
    alias(3);
  }
}

void reusedCallableScope(bool d, List<void Function(int)> callbacks) {
  if (d) { void log([int level = 0]) {} }
  callbacks.forEach((log) => log(0));
  {
    if (d) { void log([int level = 0]) {} }
    callbacks.forEach((log) => log(0));
  }
  if (d) { void log([int level = 0]) {} }
  for (final log in callbacks) { log(0); }
}

void transitive([int value = 2]) {}
void transitiveLocalScope() {
  const x = 1;
  const y = x;
  {
    const x = 2;
    transitive(y);
    void local([int value = y]) {}
    local(2);
  }
}

class ScopeReceiver { static const limit = 2; }
const limit = 1;
extension on ScopeReceiver {
  void run() {
    void local([int value = limit]) {}
    local(2);
  }
}

class DefaultBase {
  void method([int value = 4]) {}
}
class DefaultDerived extends DefaultBase {
  void run() { super.method(5); }
}
class DefaultReceiver {}
extension DefaultExtension on DefaultReceiver {
  void method([int value = 5]) {}
}
void alternateDefaultCalls(DefaultReceiver receiver) {
  void local([int value = 1]) {}
  local.call(2);
  receiver..method(6);
  DefaultExtension(receiver).method(6);
}
