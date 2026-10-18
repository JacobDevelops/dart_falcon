import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart' as hooks;
final countProvider = Provider<int>((ref) => 0);
class Counter extends Notifier<int> { void increment() {} void reset() {} int get value => 0; }
class AsyncCounter extends AsyncNotifier<int> { void refresh() {} }
final counterProvider = Provider<Counter>((ref) => Counter());
final asyncProvider = Provider<AsyncCounter>((ref) => AsyncCounter());
class Screen extends ConsumerWidget {
  Object build(Object context, WidgetRef ref) {
    ref.read(counterProvider).increment(); /* expect: avoid-calling-notifier-members-inside-build */
    final counter = ref.watch(counterProvider);
    counter.increment(); /* expect: avoid-calling-notifier-members-inside-build */
    counter.reset(); /* expect: avoid-calling-notifier-members-inside-build */
    ref.watch(asyncProvider).refresh(); /* expect: avoid-calling-notifier-members-inside-build */
    return 0;
  }
}
class Arrow extends ConsumerWidget {
  Object build(Object context, WidgetRef ref) {
    ref.read(counterProvider).increment(); /* expect: avoid-calling-notifier-members-inside-build */
    return 0;
  }
}
Object consumer() => Consumer(builder: (context, ref, child) {
  ref.read(counterProvider).increment(); /* expect: avoid-calling-notifier-members-inside-build */
  return 0;
});
class HookScreen extends hooks.HookConsumerWidget {
  Object build(Object context, hooks.WidgetRef ref) {
    ref.read(counterProvider).increment(); /* expect: avoid-calling-notifier-members-inside-build */
    return 0;
  }
}
