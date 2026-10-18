import 'dart:async';
void check(Future<int> future, Stream<int> stream, dynamic unknown) {
  try {} catch (error, stackTrace) {}
  try {} catch (exception) {}
  try {} catch (ignored) {}
  try {} catch (_) {}
  try {} catch (__) {}
  future.catchError((_) => 0);
  future.catchError((error) => 0);
  stream.handleError((exception) {});
  unknown.catchError((e) {});
  future.then((value) => value);
}
