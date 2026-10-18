import 'dart:async';
import 'dart:async' as async;

Future<int> fetch() async => 1;
Future<int> guarded() async {
  try {
    return fetch(); /* expect: prefer-return-await */
  } catch (_) { return 0; }
}
Future<int> typedCatch() async {
  try {
    return (fetch()); /* expect: prefer-return-await */
  } on Exception { return 0; }
}
Future<int> branch(bool condition) async {
  try {
    if (condition) {
      return fetch(); /* expect: prefer-return-await */
    }
    return 0;
  } catch (_) { return 0; }
}
Future<int> nestedTry() async {
  try {
    try { return fetch(); /* expect: prefer-return-await */ }
    finally { print('cleanup'); }
  } catch (_) { return 0; }
}
async.Future<int> prefixed(async.Future<int> value) async {
  try { return value; /* expect: prefer-return-await */ }
  catch (_) { return 0; }
}
