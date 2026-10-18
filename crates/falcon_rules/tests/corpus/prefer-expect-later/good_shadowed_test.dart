import 'package:test/test.dart' as t;
class Local {
  dynamic get completes => 1;
  dynamic completion(dynamic value) => 1;
  void check() {
    t.expect(1, completes);
    t.expect(1, completion(1));
  }
}
void main() {
  final completes = 1;
  t.expect(1, completes);
  dynamic completion(dynamic value) => value;
  t.expect(1, completion(1));
}
