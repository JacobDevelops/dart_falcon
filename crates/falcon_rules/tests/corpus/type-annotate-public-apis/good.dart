// @dart = 3.13

const version = '1.0';
final pi = 3.14;
final name = 'falcon';
var _private = 1;
int count = 0;

void install(String id) {}

_helper(x) {}

typedef void Handler(Object event);

class Widget {
  final int size;
  static const limit = 4;
  final title = 'widget';
  var _cache;
  Widget(this.size, {String? key});
  Widget._internal(x) : size = 0;
  _render(x) {}
  void update(int value, [_]) {}
  int get label => size;
  set label(int v) {}
  Widget operator +(Widget other) => this;
  operator []=(int i, int v) {}
}

class _Hidden {
  _Hidden(x);
}

enum Kind {
  a(1);
  final int v;
  const Kind(this.v);
}

void main() {
  local(x) {}
  local(1);
  print([_private, _Hidden(1), _helper]);
}

// Function-typed formals are not regular formals; the public-API helper stops
// at that boundary even when their nested parameter types are omitted.
void nestedTypeName(void g(int)) {}
void nestedUntyped(void cb(x)) {}
typedef void NestedCallback(void cb(x));
typedef ModernCallback = void Function(dynamic x);

class TypedPrimary(var int x);
class FinalPrimary(final String value);
class _PrivatePrimary(x);
class PrivateNamedPrimary._hidden(x);
class WildcardPrimary(_, __);
extension type TypedRepresentation(int value) {}

// Enum constructors are not public APIs, including declaring parameters.
enum PlainPrimaryEnum(p) {
  one(0)
}
enum DeclaringPrimaryEnum(final p) {
  one(0)
}
