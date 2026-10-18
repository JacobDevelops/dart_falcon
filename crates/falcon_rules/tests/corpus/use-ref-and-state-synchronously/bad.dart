import 'package:flutter_riverpod/flutter_riverpod.dart';
final provider = Object();
Future<void> pause() async {}
abstract class Count extends Notifier<int> {
  Future<void> increment() async {
    await pause();
    state += 1; /* expect: use-ref-and-state-synchronously */
  }
  Future<void> explicitThis() async {
    await pause();
    this.state = 2; /* expect: use-ref-and-state-synchronously */
  }
  Future<void> read() async {
    await pause();
    ref.read(provider); /* expect: use-ref-and-state-synchronously */
  }
  Future<void> staleGuard() async {
    if (!ref.mounted) return;
    await pause();
    state = 3; /* expect: use-ref-and-state-synchronously */
  }
  Future<void> secondGap() async {
    await pause();
    if (!ref.mounted) return;
    await pause();
    state = 4; /* expect: use-ref-and-state-synchronously */
  }
}
abstract class AsyncCount extends AsyncNotifier<int> {
  Future<void> increment() async {
    await pause();
    state = 5; /* expect: use-ref-and-state-synchronously */
  }
}
abstract class StreamCount extends StreamNotifier<int> {
  Future<void> increment() async {
    await pause();
    state = 6; /* expect: use-ref-and-state-synchronously */
  }
}
