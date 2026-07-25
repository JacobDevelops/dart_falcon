// Good examples for avoid-unused-parameters rule
// Using all declared parameters

void logMessage(String message) {
  print(message);
}

void processData(int value, String name, bool flag) {
  print(value);
  print(name);
  if (flag) {
    print("flagged");
  }
}

void usingAllParameters(String a, String b, String c) {
  print(a);
  print(b);
  print(c);
}

class MyClass {
  void method(int id, String name) {
    print(id);
    print(name);
  }

  void anotherMethod(int a, int b, int c) {
    print(a + b + c);
  }

  static void staticMethod(String key, String value) {
    print("$key: $value");
  }
}

typedef Callback = void Function(String result);

void functionWithCallback(Callback cb) {
  cb("result");
}

Future<void> asyncFunction(int timeout) {
  return Future.delayed(Duration(milliseconds: timeout));
}

extension StringExt on String {
  void customMethod(String value) {
    print(value);
  }

  int compareWith(String other) {
    return length.compareTo(other.length);
  }
}

void functionUsingInNestedScope(String param) {
  void inner() {
    print(param);
  }
  inner();
}

// Parameter used only inside a closure with a BLOCK body (regression: the
// reference collector must descend into block-bodied closures).
List<int> mapInClosure(int factor) {
  return [1, 2, 3].map((x) {
    return x * factor;
  }).toList();
}

// Parameter used only inside a switch-case body.
String describe(int code) {
  switch (code) {
    case 200:
      return 'ok';
    default:
      return 'code $code';
  }
}

// Parameter used only inside a cascade section.
void configure(Object target, String label) {
  target
    ..toString()
    ..hashCode.toString()
    ..runtimeType.toString();
  print(label);
}

class Overrides {
  // @override methods keep the supertype's parameter list; an unused param here
  // is not the author's to remove, so it is exempt.
  @override
  void didUpdateWidget(Object oldWidget) {
    print('updated');
  }
}

// All-underscore parameter names are conventional "unused" markers.
void ignoresArgs(int __, String ___) {
  print('ignored');
}

// Parameter used only inside a map-comprehension iterable
// (`{ for (..) k: v }` lives in `Map.elements`, not `Map.entries`).
Map<String, int> buildLookup(List<String> keys) {
  return {for (final k in keys) k: k.length};
}

// Dart 3 destructuring: the parameter is read only in the initializer of a
// pattern declaration / pattern assignment / pattern for-in, or inside a
// labeled statement.
String recordPatternDecl(Object shape) {
  final (label, sides) = switch (shape) {
    int() => ('int', 0),
    _ => ('other', 4),
  };

  return '$label$sides';
}

String recordPatternAssign(Object shape) {
  String label;
  int sides;
  (label, sides) = switch (shape) {
    int() => ('int', 0),
    _ => ('other', 4),
  };

  return '$label$sides';
}

int patternForIn(List<(int, int)> pairs) {
  var total = 0;
  for (final (a, b) in pairs) {
    total += a + b;
  }

  return total;
}

int labeledLoop(int value) {
  outer:
  for (var i = 0; i < 3; i++) {
    if (value > 1) break outer;
  }

  return 0;
}

// Parameter used only as a bare generic tear-off (`fn<int>`, no call).
void useTearOff(T Function<T>(T) fn) {
  final g = fn<int>;
  print(g);
}

// A genuine interpolated read still counts as a use, in both `$id` and
// `${expr}` form.
class InterpolationReads {
  String simple(int count) => "n=$count";

  String complex(int count) => "n=${count + 1}";
}

// A Dart 3 pattern assignment writes to its targets, which counts as a use
// just as a plain `x = …` assignment does.
class PatternAssignTarget {
  void reassign(int a, int b) {
    (a, b) = (1, 2);
  }
}
