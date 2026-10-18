import 'dart:async';
import 'dart:async' as async;

Future<Future<int>>? first; /* expect: avoid-nested-futures */
FutureOr<Future<int>>? second; /* expect: avoid-nested-futures */
FutureOr<FutureOr<int>>? third; /* expect: avoid-nested-futures */
Future<FutureOr<String>>? fourth; /* expect: avoid-nested-futures */
async.Future<async.Future<int>>? prefixed; /* expect: avoid-nested-futures */
void consume(Future<Future<void>> value) {} /* expect: avoid-nested-futures */
