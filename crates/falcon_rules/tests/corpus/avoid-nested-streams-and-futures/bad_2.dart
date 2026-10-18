import 'dart:async';

typedef Result<T> = Future<T>;
typedef Events<T> = Stream<T>;
Result<Stream<int>>? result; /* expect: avoid-nested-streams-and-futures */
Events<Future<int>>? events; /* expect: avoid-nested-streams-and-futures */
