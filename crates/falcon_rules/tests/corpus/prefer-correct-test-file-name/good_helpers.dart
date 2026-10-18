import 'package:test/test.dart' as t;
void helper() {}
class Local {
  void test(String name, dynamic callback) {}
  void run() { test('local', () {}); }
}
void local() {
  void group(String name, dynamic callback) {}
  group('local group', () {});
  final test = (String name, dynamic callback) {};
  test('local closure', () {});
}
