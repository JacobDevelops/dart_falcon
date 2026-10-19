import 'dart:async';

Stream<int>? sequence;
Future<int>? result;
FutureOr<int>? maybe;
Stream<Stream<int>>? sequences;
Future<Future<int>>? nestedFuture;
Future<List<Stream<int>>>? batches;
Stream<List<Future<int>>>? jobs;
Future<Stream<int> Function()>? callback;
