import 'dart:async';
void check(Future<int> future, Stream<int> stream) {
  try {} catch (err) {} /* expect: prefer-correct-error-name */
  future.then((value) => value, onError: (e) {}); /* expect: prefer-correct-error-name */
  future.catchError((failure) => 0); /* expect: prefer-correct-error-name */
  stream.listen((value) {}, onError: (err, stack) {}); /* expect: prefer-correct-error-name */
  stream.handleError((err) {}); /* expect: prefer-correct-error-name */
}
