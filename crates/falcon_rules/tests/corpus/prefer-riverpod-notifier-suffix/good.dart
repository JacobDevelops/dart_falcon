import 'package:riverpod/riverpod.dart' as rp;
class CounterNotifier extends rp.Notifier<int> {}
class AsyncCounterNotifier extends rp.AsyncNotifier<int> {}
class StreamCounterNotifier extends rp.StreamNotifier<int> {}
class AutoCounterNotifier extends rp.AutoDisposeNotifier<int> {}
class ChildNotifier extends CounterNotifier {}
class _PrivateNotifier extends rp.Notifier<int> {}
class Notifier<T> {}
class Unrelated extends Notifier<int> {}
class AsyncNotifier<T> {}
class UnrelatedAsync extends AsyncNotifier<int> {}
class Ordinary {}
class Missing extends UnknownNotifier<int> {}
class FamilyCounterNotifier extends rp.FamilyNotifier<int, String> {}
class AsyncFamilyCounterNotifier extends rp.FamilyAsyncNotifier<int, String> {}
class StreamFamilyCounterNotifier extends rp.FamilyStreamNotifier<int, String> {}
class AutoFamilyCounterNotifier extends rp.AutoDisposeFamilyNotifier<int, String> {}
class AutoAsyncFamilyCounterNotifier extends rp.AutoDisposeFamilyAsyncNotifier<int, String> {}
class AutoStreamFamilyCounterNotifier extends rp.AutoDisposeFamilyStreamNotifier<int, String> {}
class BuildlessStreamCounterNotifier extends rp.BuildlessStreamNotifier<int> {}
class AnyCounterNotifier extends rp.AnyNotifier<int, int> {}
class FamilyNotifier<T, Arg> {}
class UnrelatedFamily extends FamilyNotifier<int, String> {}
class BuildlessStreamNotifier<T> {}
class UnrelatedBuildless extends BuildlessStreamNotifier<int> {}
class AnyNotifier<StateT, ValueT> {}
class UnrelatedAny extends AnyNotifier<int, int> {}
