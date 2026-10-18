import 'dart:async';
import 'package:riverpod/riverpod.dart';
final ownedStream = StreamProvider((ref) {
  final controller = StreamController<int>();
  return controller.stream;
});
final autoTimer = Provider.autoDispose((ref) => Timer(Duration.zero, () {}));
final registeredTimer = Provider((ref) {
  final timer = Timer(Duration.zero, () {});
  ref.onDispose(timer.cancel);
  return timer;
});
