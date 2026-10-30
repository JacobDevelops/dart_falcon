abstract class Notifier<T> {}
abstract class AsyncNotifier<T> {}
abstract class StreamNotifier<T> {}
abstract class AutoDisposeNotifier<T> extends Notifier<T> {}
abstract class StateNotifier<T> {}
abstract class BuildlessNotifier<T> {}
abstract class BuildlessAsyncNotifier<T> {}
abstract class BuildlessStreamNotifier<T> {}
abstract class FamilyNotifier<T, Arg> {}
abstract class FamilyAsyncNotifier<T, Arg> {}
abstract class FamilyStreamNotifier<T, Arg> {}
abstract class AutoDisposeFamilyNotifier<T, Arg> {}
abstract class AutoDisposeFamilyAsyncNotifier<T, Arg> {}
abstract class AutoDisposeFamilyStreamNotifier<T, Arg> {}
abstract class AnyNotifier<StateT, ValueT> {}
