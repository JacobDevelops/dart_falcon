import 'package:flutter_riverpod/flutter_riverpod.dart';
final provider = Object();
Future<void> pause() async {}
abstract class Example extends ConsumerState<Object> {
  Future<void> afterAwait() async {
    await pause();
    ref.read(provider); /* expect: use-ref-read-synchronously */
  }
  Future<void> explicitThis() async {
    await pause();
    this.ref.read(provider); /* expect: use-ref-read-synchronously */
  }
  Future<void> staleGuard() async {
    if (!mounted) return;
    await pause();
    ref.read(provider); /* expect: use-ref-read-synchronously */
  }
  Future<void> branch(bool condition) async {
    if (condition) await pause();
    ref.read(provider); /* expect: use-ref-read-synchronously */
  }
  Future<void> secondGap() async {
    await pause();
    if (!mounted) return;
    await pause();
    ref.read(provider); /* expect: use-ref-read-synchronously */
  }
}
Future<void> parameter(WidgetRef ref) async {
  await pause();
  ref.read(provider); /* expect: use-ref-read-synchronously */
}
