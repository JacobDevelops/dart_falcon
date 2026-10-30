import 'package:riverpod/riverpod.dart' as rp;
import 'package:unrelated/unrelated.dart' as other;

void initialize() {}

class EmptyNotifier extends rp.Notifier<int> {
  EmptyNotifier();
}
class EmptyAsync extends rp.AsyncNotifier<int> {
  EmptyAsync() { /* Comments are not logic. */ }
}
class EmptyStream extends rp.StreamNotifier<int> {
  EmptyStream.named() {}
}
class ImplicitConstructor extends rp.AutoDisposeFamilyNotifier<int, String> {}
class EmptySuper extends rp.Notifier<int> {
  EmptySuper() : super();
}
class EmptyRedirect extends rp.Notifier<int> {
  EmptyRedirect() : this.named();
  EmptyRedirect.named();
}
class RedirectingFactory extends rp.Notifier<int> {
  RedirectingFactory._();
  factory RedirectingFactory() = RedirectingFactory._;
}
class OrdinaryNotifier extends other.Notifier<int> {
  OrdinaryNotifier() { initialize(); }
}
class OrdinaryAsync extends other.AsyncNotifier<int> {
  final int initial;
  OrdinaryAsync() : initial = 1;
}
class OrdinaryStream extends other.StreamNotifier<int> {
  OrdinaryStream() { initialize(); }
}
class OrdinaryFamily extends other.FamilyNotifier<int, String> {
  OrdinaryFamily() { initialize(); }
}
class StateNotifierLegacy extends rp.StateNotifier<int> {
  StateNotifierLegacy() : super(1) { initialize(); }
}
class Notifier<T> {}
class LocalLookalike extends Notifier<int> {
  LocalLookalike() { initialize(); }
}
class BuildInitialization extends rp.Notifier<int> {
  int build() { initialize(); return 1; }
}
class EmptyBuildlessStream extends rp.BuildlessStreamNotifier<int> {
  EmptyBuildlessStream();
}
class EmptyAny extends rp.AnyNotifier<int, int> {
  EmptyAny() { /* No initialization logic. */ }
}
class IndirectLegacy extends StateNotifierLegacy {
  IndirectLegacy() { initialize(); }
}
class BuildlessStreamNotifier<T> {}
class OrdinaryBuildlessStream extends BuildlessStreamNotifier<int> {
  OrdinaryBuildlessStream() { initialize(); }
}
class AnyNotifier<StateT, ValueT> {}
class OrdinaryAny extends AnyNotifier<int, int> {
  OrdinaryAny() { initialize(); }
}
