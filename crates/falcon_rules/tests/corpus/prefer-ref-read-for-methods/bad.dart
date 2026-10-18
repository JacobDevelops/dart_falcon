import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart' as hooks;
final countProvider = Provider<int>((ref) => 0);
class Counter extends Notifier<int> { void increment() {} void reset() {} int get value => 0; }
class AsyncCounter extends AsyncNotifier<int> { void refresh() {} }
final counterProvider = Provider<Counter>((ref) => Counter());
final asyncProvider = Provider<AsyncCounter>((ref) => AsyncCounter());
void actions(WidgetRef ref) {
  ref.watch(counterProvider).increment(); /* expect: prefer-ref-read-for-methods */
  ref.watch(counterProvider).reset(); /* expect: prefer-ref-read-for-methods */
  final increment = ref.watch(counterProvider).increment; /* expect: prefer-ref-read-for-methods */
  ref.watch<Counter>(counterProvider).increment(); /* expect: prefer-ref-read-for-methods */
  (ref.watch(counterProvider)).reset(); /* expect: prefer-ref-read-for-methods */
  ref.watch(asyncProvider).refresh(); /* expect: prefer-ref-read-for-methods */
}
