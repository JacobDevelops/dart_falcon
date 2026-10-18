import 'package:flutter_riverpod/flutter_riverpod.dart';
class Counter extends Notifier<int> { void increment() {} }
class AutoCounter extends AutoDisposeNotifier<int> { void increment() {} }
class StreamCounter extends StreamNotifier<int> { void increment() {} }
class LegacyCounter extends StateNotifier<int> { LegacyCounter() : super(0); void increment() {} }
final provider = NotifierProvider<Counter, int>(Counter.new);
final autoProvider = Provider<AutoCounter>((ref) => AutoCounter());
final streamProvider = Provider<StreamCounter>((ref) => StreamCounter());
final legacyProvider = Provider<LegacyCounter>((ref) => LegacyCounter());
class Screen extends ConsumerWidget {
  Object build(Object context, WidgetRef ref) {
    ref.read(provider.notifier).increment(); /* expect: avoid-calling-notifier-members-inside-build */
    ref.read(autoProvider).increment(); /* expect: avoid-calling-notifier-members-inside-build */
    ref.read(streamProvider).increment(); /* expect: avoid-calling-notifier-members-inside-build */
    ref.read(legacyProvider).increment(); /* expect: avoid-calling-notifier-members-inside-build */
    ref.read(provider.notifier)
      ..increment() /* expect: avoid-calling-notifier-members-inside-build */
      ..increment(); /* expect: avoid-calling-notifier-members-inside-build */
    return 0;
  }
}
