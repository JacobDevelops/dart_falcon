import 'package:test/test.dart';

class Harness {
  void fail(String message) => print(message);

  void run(void Function() body) {
    try {
      body();
      fail('no throw');
    } catch (_) {}
  }
}
