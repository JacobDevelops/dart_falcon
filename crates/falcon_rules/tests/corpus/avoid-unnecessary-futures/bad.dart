import 'dart:async';
import 'dart:async' as async;

Future<int> immediate() async => 1; /* expect: avoid-unnecessary-futures */
Future<String> text() async => 'value'; /* expect: avoid-unnecessary-futures */
async.Future<bool> flag() async => true; /* expect: avoid-unnecessary-futures */
Future<double> number() async { /* expect: avoid-unnecessary-futures */
  return 1.5;
}
Future<int> parameter(int value) async => value; /* expect: avoid-unnecessary-futures */
Future<int> local() async { /* expect: avoid-unnecessary-futures */
  final value = 1;
  return value;
}
