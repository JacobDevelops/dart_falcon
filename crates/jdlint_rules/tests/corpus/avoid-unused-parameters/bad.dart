// Test cases for avoid-unused-parameters rule
// Flags function/method parameters that are declared but never referenced

void logMessage(String message, String unused) { /* expect: avoid-unused-parameters */
  print(message);
}

void processData(int value, String ignore, bool flag) { /* expect: avoid-unused-parameters */
  print(value);
  if (flag) {
    print("flagged");
  }
}

void simpleUnused(String name, String unused) { /* expect: avoid-unused-parameters */ /* expect: avoid-unused-parameters */
  return;
}

class MyClass {
  void method(int id, String unused) { /* expect: avoid-unused-parameters */
    print(id);
  }

  void anotherMethod(int a, int b, int c) { /* expect: avoid-unused-parameters */ /* expect: avoid-unused-parameters */
    print(a);
  }

  static void staticMethod(String key, String ignored) { /* expect: avoid-unused-parameters */
    print(key);
  }
}

typedef Callback = void Function(String result, String unused);

void functionWithCallback(Callback cb, String unused) { /* expect: avoid-unused-parameters */
  cb("result");
}

Future<void> asyncFunction(int timeout, String unused) { /* expect: avoid-unused-parameters */
  return Future.delayed(Duration(milliseconds: timeout));
}

extension StringExt on String {
  void customMethod(String value, String unused) { /* expect: avoid-unused-parameters */
    print(value);
  }
}
