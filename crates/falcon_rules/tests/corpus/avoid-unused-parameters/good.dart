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
