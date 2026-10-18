import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter/widgets.dart';
final provider = Object();
Future<void> pause() async {}
abstract class Example extends ConsumerState<Object> {
  Future<void> beforeAwait() async {
    ref.read(provider);
    await pause();
  }
  Future<void> guarded() async {
    await pause();
    if (mounted) ref.read(provider);
  }
  Future<void> earlyReturn() async {
    await pause();
    if (!mounted) return;
    ref.read(provider);
  }
  Future<void> nested() async {
    await pause();
    void callback() { ref.read(provider); }
    callback();
  }
  Future<void> contextGuard(BuildContext context) async {
    await pause();
    if (!context.mounted) return;
    ref.read(provider);
  }
}
class Unrelated {
  Object read(Object provider) => provider;
}
Future<void> unrelated(Unrelated ref) async {
  await pause();
  ref.read(provider);
}
abstract class Count extends Notifier<int> {
  Future<void> notifierHandledElsewhere() async {
    await pause();
    ref.read(provider);
  }
}
