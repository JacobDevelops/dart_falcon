class Notifier<T> {
  Notifier();
  Notifier.seed(int value);
  late T state;
}
class AsyncNotifier<T> {}
class StreamNotifier<T> {}
class AutoDisposeNotifier<T> {}
class AutoDisposeAsyncNotifier<T> {}
class AutoDisposeStreamNotifier<T> {}
class FamilyNotifier<T, Arg> {}
class FamilyAsyncNotifier<T, Arg> {}
class FamilyStreamNotifier<T, Arg> {}
class AutoDisposeFamilyNotifier<T, Arg> {}
class AutoDisposeFamilyAsyncNotifier<T, Arg> {}
class AutoDisposeFamilyStreamNotifier<T, Arg> {}
class $Notifier<T> {}
class $AsyncNotifier<T> {}
class $StreamNotifier<T> {}
class StateNotifier<T> {
  StateNotifier(T value);
}
class BuildlessNotifier<T> {}
class BuildlessAsyncNotifier<T> {}
class BuildlessStreamNotifier<T> {}
class AnyNotifier<StateT, ValueT> {}
