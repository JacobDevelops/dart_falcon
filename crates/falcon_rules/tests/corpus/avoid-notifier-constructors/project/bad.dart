import 'package:riverpod/riverpod.dart' as rp;
import 'package:flutter_riverpod/flutter_riverpod.dart' as flutter;
import 'package:hooks_riverpod/hooks_riverpod.dart' as hooks;

void initialize() {}
int seed() => 1;

class AsyncBody extends rp.AsyncNotifier<int> {
  AsyncBody() { initialize(); } /* expect: avoid-notifier-constructors */
}
class StreamBody extends rp.StreamNotifier<int> {
  StreamBody.named() { initialize(); } /* expect: avoid-notifier-constructors */
}
class AutoDisposeBody extends rp.AutoDisposeNotifier<int> {
  AutoDisposeBody() { initialize(); } /* expect: avoid-notifier-constructors */
}
class AutoDisposeAsyncBody extends flutter.AutoDisposeAsyncNotifier<int> {
  AutoDisposeAsyncBody() { initialize(); } /* expect: avoid-notifier-constructors */
}
class AutoDisposeStreamBody extends hooks.AutoDisposeStreamNotifier<int> {
  AutoDisposeStreamBody() { initialize(); } /* expect: avoid-notifier-constructors */
}
class FamilyBody extends rp.FamilyNotifier<int, String> {
  FamilyBody() { initialize(); } /* expect: avoid-notifier-constructors */
}
class AsyncFamilyBody extends rp.FamilyAsyncNotifier<int, String> {
  AsyncFamilyBody() { initialize(); } /* expect: avoid-notifier-constructors */
}
class StreamFamilyBody extends rp.FamilyStreamNotifier<int, String> {
  StreamFamilyBody() { initialize(); } /* expect: avoid-notifier-constructors */
}
class AutoFamilyBody extends rp.AutoDisposeFamilyNotifier<int, String> {
  AutoFamilyBody() { initialize(); } /* expect: avoid-notifier-constructors */
}
class AutoAsyncFamilyBody extends rp.AutoDisposeFamilyAsyncNotifier<int, String> {
  AutoAsyncFamilyBody() { initialize(); } /* expect: avoid-notifier-constructors */
}
class AutoStreamFamilyBody extends rp.AutoDisposeFamilyStreamNotifier<int, String> {
  AutoStreamFamilyBody() { initialize(); } /* expect: avoid-notifier-constructors */
}
abstract class Intermediate<T> extends rp.Notifier<T> {}
class IndirectBody extends Intermediate<int> {
  IndirectBody() { initialize(); } /* expect: avoid-notifier-constructors */
}
class FieldInitializer extends rp.Notifier<int> {
  final int initial;
  FieldInitializer() : initial = seed(); /* expect: avoid-notifier-constructors */
}
class InitializingFormal extends rp.Notifier<int> {
  final int initial;
  InitializingFormal(this.initial); /* expect: avoid-notifier-constructors */
}
class AssertionInitializer extends rp.Notifier<int> {
  AssertionInitializer(int value) : assert(value > 0); /* expect: avoid-notifier-constructors */
}
class SuperInitializer extends rp.Notifier<int> {
  SuperInitializer() : super.seed(seed()); /* expect: avoid-notifier-constructors */
}
class RedirectArguments extends rp.Notifier<int> {
  RedirectArguments() : this.seeded(seed()); /* expect: avoid-notifier-constructors */
  RedirectArguments.seeded(int value);
}
class FactoryLogic extends rp.Notifier<int> {
  FactoryLogic._();
  factory FactoryLogic() { initialize(); return FactoryLogic._(); } /* expect: avoid-notifier-constructors */
  factory FactoryLogic.arrow() => FactoryLogic._(); /* expect: avoid-notifier-constructors */
}
class GeneratedBody extends rp.$Notifier<int> {
  GeneratedBody() { initialize(); } /* expect: avoid-notifier-constructors */
}
class BuildlessBody extends rp.BuildlessNotifier<int> {
  BuildlessBody() { initialize(); } /* expect: avoid-notifier-constructors */
}
class BuildlessAsyncBody extends rp.BuildlessAsyncNotifier<int> {
  BuildlessAsyncBody() { initialize(); } /* expect: avoid-notifier-constructors */
}
class BuildlessStreamBody extends rp.BuildlessStreamNotifier<int> {
  BuildlessStreamBody() { initialize(); } /* expect: avoid-notifier-constructors */
}
class AnyBody extends rp.AnyNotifier<int, int> {
  AnyBody() { initialize(); } /* expect: avoid-notifier-constructors */
}
