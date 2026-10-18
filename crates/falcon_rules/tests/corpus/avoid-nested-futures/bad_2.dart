import 'dart:async';

typedef Result<T> = Future<T>;
Result<Future<int>>? result; /* expect: avoid-nested-futures */
Future<Result<int>>? nestedAlias; /* expect: avoid-nested-futures */
