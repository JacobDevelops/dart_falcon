// A type parameter's bound must be a `Future` subtype: unlike the awaited
// type itself, a `FutureOr` or `dynamic` bound is not exempt.
import 'dart:async';

Future<void> bounds<FO extends FutureOr<int>, U, V extends U, D extends dynamic>(
  FO futureOr,
  FO? nullableFutureOr,
  U unbounded,
  V nested,
  D untyped,
) async {
  await futureOr; /* expect: await-only-futures */
  await nullableFutureOr; /* expect: await-only-futures */
  await unbounded; /* expect: await-only-futures */
  await nested; /* expect: await-only-futures */
  await untyped; /* expect: await-only-futures */
}
