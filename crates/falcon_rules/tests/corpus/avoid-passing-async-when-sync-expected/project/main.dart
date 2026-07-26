import 'callbacks.dart' as callbacks;
import 'callbacks.dart' show SyncApi, AsyncApi;

void check() {
  callbacks.syncCallback(() async {}); /* expect: avoid-passing-async-when-sync-expected */
  callbacks.asyncCallback(() async {});
  SyncApi().run(() async {}); /* expect: avoid-passing-async-when-sync-expected */
  AsyncApi().run(() async {});
}
