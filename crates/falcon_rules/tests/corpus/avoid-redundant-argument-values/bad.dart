void configure({bool verbose = false, int retries = 3, String mode = 'fast'}) {}

void positional(int a, [int b = 1, int c = 2]) {}

const defaultLimit = 10;
void limited({int limit = defaultLimit}) {}

class Widget {
  const Widget({this.size = 1.0, this.label = ''});
  Widget.named({int count = 0}) : size = 0, label = '';
  factory Widget.redirect({double size}) = Widget;
  final double size;
  final String label;

  static const zero = 0;
  void resize({int by = zero, bool animate = true}) {}

  void selfCall() {
    resize(animate: true); /* expect: avoid-redundant-argument-values */
  }
}

enum Level {
  low(weight: 1), /* expect: avoid-redundant-argument-values */
  high(weight: 5);

  const Level({this.weight = 1});
  final int weight;
}

class Marker {
  const Marker([this.value = 'x']);
  final String value;
}

@Marker('x') /* expect: avoid-redundant-argument-values */
void annotated() {}

void main() {
  configure(verbose: false); /* expect: avoid-redundant-argument-values */
  configure(retries: 3); /* expect: avoid-redundant-argument-values */
  configure(mode: 'fast', verbose: true); /* expect: avoid-redundant-argument-values */
  positional(0, 5, 2); /* expect: avoid-redundant-argument-values */
  positional(0, 1); /* expect: avoid-redundant-argument-values */
  limited(limit: 10); /* expect: avoid-redundant-argument-values */
  Widget(size: 1.0); /* expect: avoid-redundant-argument-values */
  Widget(size: 1); /* expect: avoid-redundant-argument-values */
  Widget.named(count: 0); /* expect: avoid-redundant-argument-values */
  Widget.redirect(size: 1.0); /* expect: avoid-redundant-argument-values */
  const Widget(label: ''); /* expect: avoid-redundant-argument-values */
  final w = Widget();
  w.resize(by: 0); /* expect: avoid-redundant-argument-values */
  w.resize(animate: !false); /* expect: avoid-redundant-argument-values */
}

void nullable({String? name}) {}

class Box<E> {
  void fill(E value, [int start = 0]) {}
  void clear([int start = 0]) {}
}

void more(Box<int> box) {
  nullable(name: null); /* expect: avoid-redundant-argument-values */
  box.clear(0); /* expect: avoid-redundant-argument-values */
  void local({int x = 1}) {}
  local(x: 1); /* expect: avoid-redundant-argument-values */
  final closure = (int x, {int y = 0}) => x + y;
  closure(1, y: 0); /* expect: avoid-redundant-argument-values */
}

const lexicalDefault = 2;
void capturedLocalDefault() {
  const lexicalDefault = 1;
  void local([int value = lexicalDefault]) {}
  final alias = local;
  local(1); /* expect: avoid-redundant-argument-values */
  alias(1); /* expect: avoid-redundant-argument-values */
  {
    const lexicalDefault = 3;
    local(1); /* expect: avoid-redundant-argument-values */
    alias(1); /* expect: avoid-redundant-argument-values */
  }
}

const anonymousLimit = 1;
class AnonymousReceiver {}
extension on AnonymousReceiver {
  void method([int value = anonymousLimit]) {}
}
class AnonymousCaller {
  static const anonymousLimit = 2;
  void run(AnonymousReceiver receiver) {
    receiver.method(1); /* expect: avoid-redundant-argument-values */
  }
}

void transitiveLocalScope() {
  const x = 1;
  const y = x;
  {
    const x = 2;
    void local([int value = y]) {}
    local(1); /* expect: avoid-redundant-argument-values */
  }
}

class ScopeReceiver { static const limit = 2; }
const limit = 1;
extension on ScopeReceiver {
  void run() {
    void local([int value = limit]) {}
    local(1); /* expect: avoid-redundant-argument-values */
  }
}

class DefaultBase {
  void method([int value = 4]) {}
}
class DefaultDerived extends DefaultBase {
  void run() {
    super.method(4); /* expect: avoid-redundant-argument-values */
  }
}
class DefaultReceiver {}
extension DefaultExtension on DefaultReceiver {
  void method([int value = 5]) {}
}
void alternateDefaultCalls(DefaultReceiver receiver) {
  void local([int value = 1]) {}
  local.call(1); /* expect: avoid-redundant-argument-values */
  receiver..method(5); /* expect: avoid-redundant-argument-values */
  DefaultExtension(receiver).method(5); /* expect: avoid-redundant-argument-values */
}

const double contextualTopOne = 1;
const double contextualTopZero = -0;
class ContextualDefaults {
  static const double one = 1;
  static const double zero = -0;
}
void contextualOne([num value = 1.0]) {}
void contextualZero([num value = -0.0]) {}
void contextualConstants() {
  const double localOne = 1;
  const double localZero = -0;
  contextualOne(contextualTopOne); /* expect: avoid-redundant-argument-values */
  contextualOne(ContextualDefaults.one); /* expect: avoid-redundant-argument-values */
  contextualOne(localOne); /* expect: avoid-redundant-argument-values */
  contextualZero(contextualTopZero); /* expect: avoid-redundant-argument-values */
  contextualZero(ContextualDefaults.zero); /* expect: avoid-redundant-argument-values */
  contextualZero(localZero); /* expect: avoid-redundant-argument-values */
}
