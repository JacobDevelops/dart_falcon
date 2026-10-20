import 'dart:async';
Future<int> forwarding() => Future<int>.value(1);
Future<int> awaited() async => await Future<int>.value(1);
Future<int> flatten(Future<int> value) async => value;
Future<int> union(FutureOr<int> value) async => value;
Future<dynamic> unknown(dynamic value) async => value;
Future<int> throwing() async { throw StateError('failed'); }
Future<int> canThrow() async => int.parse('1');
Future<int> asserts(int? value) async => value!;
Future<int> effects() async { print('effect'); return 1; }
Future<void> main() async {}
abstract class Contract { Future<int> value(); }
class Implementation implements Contract {
  @override
  Future<int> value() async => 1;
}
class Member {
  Future<int> value() async => 1;
}
Future<void> empty() async {}
Stream<int> generator() async* { yield 1; }
void callback() {
  final action = () async => 1;
  print(action);
}
int get dangerous => throw StateError('failed');
Future<int> getter() async => dangerous;
Future<int> interpolation(Object value) async {
  final text = '$value';
  return 1;
}
Future<int> iteration(Stream<int> values) async {
  await for (final value in values) {}
  return 1;
}
Future<int> destructureList(List<int> values) async {
  final [value] = values;
  return value;
}
class ThrowingGetter {
  int get value => throw StateError('failed');
}
Future<int> destructureObject(ThrowingGetter object) async {
  final ThrowingGetter(:value) = object;
  return value;
}
