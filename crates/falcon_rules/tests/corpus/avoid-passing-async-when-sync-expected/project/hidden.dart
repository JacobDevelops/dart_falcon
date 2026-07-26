import 'callbacks.dart' hide syncCallback;

void hiddenAndPrivate() {
  syncCallback(() async {});
  _privateCallback(() async {});
}
