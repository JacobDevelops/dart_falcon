import 'package:flutter_riverpod/flutter_riverpod.dart';
class Counter extends Notifier<int> { void increment() {} }
class AutoCounter extends AutoDisposeNotifier<int> { void increment() {} }
class StreamCounter extends StreamNotifier<int> { void increment() {} }
class LegacyCounter extends StateNotifier<int> { LegacyCounter() : super(0); void increment() {} }
final provider = NotifierProvider<Counter, int>(Counter.new);
final autoProvider = Provider<AutoCounter>((ref) => AutoCounter());
final streamProvider = Provider<StreamCounter>((ref) => StreamCounter());
final legacyProvider = Provider<LegacyCounter>((ref) => LegacyCounter());
void actions(WidgetRef ref) {
  ref.watch(provider.notifier).increment(); /* expect: prefer-ref-read-for-methods */
  ref.watch(autoProvider).increment(); /* expect: prefer-ref-read-for-methods */
  ref.watch(streamProvider).increment(); /* expect: prefer-ref-read-for-methods */
  ref.watch(legacyProvider).increment(); /* expect: prefer-ref-read-for-methods */
}
