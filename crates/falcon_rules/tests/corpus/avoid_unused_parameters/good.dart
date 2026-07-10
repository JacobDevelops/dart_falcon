// Good: all parameters are used

void example1(String message) {
  print(message);
}

void onEvent(BuildContext context, String event) {
  print('Event: $event');
  Navigator.pop(context);
}

class MyWidget {
  void onPressed(dynamic event, String message) {
    print('$message: $event');
  }
}

String processData(String data, int count) {
  return '$data x $count';
}

void multiUnused(int a, int b, int c) {
  print(a + b + c);
}

void callback(String? name, String? email) {
  if (name != null) {
    print(name);
  }
  if (email != null) {
    print(email);
  }
}

// Good: unused parameters prefixed with _

void example2(String _unused) {
  print('hello');
}

void handler(BuildContext _context, String _message) {
  print('Handled');
}

void skipParams(String _name, int _count) {
  print('Skipped');
}

// Good: `@override` methods must keep the supertype's parameter list, so an
// unused parameter there is not the author's to remove.
class MyView {
  @override
  Widget build(BuildContext context) => const Text('x');
}

// Good: `noSuchMethod` receives an `Invocation` it is free to ignore.
class Proxy {
  dynamic noSuchMethod(Invocation invocation) => null;
}

// Good: `this.` / `super.` initializing formals bind straight to a field or the
// super-constructor — the parameter is its own use, never flag them.
class Point {
  final int x;
  final int y;
  Point(this.x, this.y);
}

class ColoredPoint extends Point {
  final int color;
  ColoredPoint(super.x, super.y, this.color) : super();
}

// Good: a parameter used only inside a map comprehension's iterable counts as
// used (the walker descends into `Expr::Map.elements`).
Map<String, int> lengths(List<String> words) {
  return {for (final w in words) w: w.length};
}
