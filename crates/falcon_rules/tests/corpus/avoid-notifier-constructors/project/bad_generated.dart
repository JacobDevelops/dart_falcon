import 'package:riverpod/riverpod.dart' as rp;

part 'generated_counter.g.dart';

void initializeGenerated() {}
class GeneratedCounter extends _$Counter {
  GeneratedCounter() { initializeGenerated(); } /* expect: avoid-notifier-constructors */
}
class EmptyGeneratedCounter extends _$Counter {
  EmptyGeneratedCounter();
}
