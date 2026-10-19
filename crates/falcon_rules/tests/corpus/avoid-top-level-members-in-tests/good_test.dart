import 'package:test/test.dart';
var _count = 0;
const _value = 1;
final _values = <int>[];
void _helper() {}
class _Helper { int publicMember = 1; }
mixin _Behavior {}
enum _Choice { first }
extension _Description on int { String describe() => '$this'; }
extension on String { int get lengthTwice => length * 2; }
typedef _Callback = void Function();
void main() { var local = 0; test('case', () { local++; }); }
