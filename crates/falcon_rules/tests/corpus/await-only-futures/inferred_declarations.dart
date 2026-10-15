import 'dart:async';

var number = 1;
final text = 'text';
final forward = later;
final later = Box(1);
get topBox => forward;
var topFuture = Future<int>.value(1);
get futureGetter => topFuture;

class Box<T> {
  Box(this.value);
  final T value;
}

class Values {
  static var count = 2;
  static final boxed = Box('text');
  static get staticBox => boxed;
  static final future = Future<int>.value(1);
  final instanceBox = Box(3);
  get inferredBox => instanceBox;
  int get declaredCount => count;
}

Future<void> checkDeclarations(Values values) async {
  await number; /* expect: await-only-futures */
  await text; /* expect: await-only-futures */
  await forward; /* expect: await-only-futures */
  await topBox; // A getter without a return annotation is dynamic.
  await Values.count; /* expect: await-only-futures */
  await Values.boxed; /* expect: await-only-futures */
  await Values.staticBox;
  await values.instanceBox; /* expect: await-only-futures */
  await values.inferredBox;
  await values.declaredCount; /* expect: await-only-futures */
  await topFuture;
  await futureGetter;
  await Values.future;
}
