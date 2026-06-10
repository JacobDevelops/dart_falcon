// Bad: unused function/method parameters

void example1(String unused) { /* expect: avoid_unused_parameters */
  print('hello');
}

void onEvent(BuildContext context, String unused) { /* expect: avoid_unused_parameters */
  Navigator.pop(context);
}

class MyWidget {
  void onPressed(dynamic event, String message) { /* expect: avoid_unused_parameters */
    print('Pressed');
  }
}

String processData(String data, int count) { /* expect: avoid_unused_parameters */
  return data;
}

void multiUnused(int a, int b, int c) { /* expect: avoid_unused_parameters */ /* expect: avoid_unused_parameters */
  print(a);
}

void callback(String? name, String? email) { /* expect: avoid_unused_parameters */
  if (name != null) {
    print(name);
  }
}
