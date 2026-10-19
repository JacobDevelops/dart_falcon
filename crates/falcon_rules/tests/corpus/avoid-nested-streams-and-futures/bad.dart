import 'dart:async';
import 'dart:async' as async;

Stream<Future<int>>? first; /* expect: avoid-nested-streams-and-futures */
Future<Stream<int>>? second; /* expect: avoid-nested-streams-and-futures */
Stream<FutureOr<int>>? third; /* expect: avoid-nested-streams-and-futures */
FutureOr<Stream<String>>? fourth; /* expect: avoid-nested-streams-and-futures */
async.Stream<async.Future<int>>? prefixed; /* expect: avoid-nested-streams-and-futures */
void consume(Future<Stream<void>> value) {} /* expect: avoid-nested-streams-and-futures */
