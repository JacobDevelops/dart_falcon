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
