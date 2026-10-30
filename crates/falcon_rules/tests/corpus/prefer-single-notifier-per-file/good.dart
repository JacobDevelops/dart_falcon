import 'package:riverpod/riverpod.dart' as r;
abstract class Only extends r.Notifier<int> {}
class Notifier<T> {}
class Lookalike extends Notifier<int> {}
class AsyncNotifier<T> {}
class OtherLookalike extends AsyncNotifier<int> {}
class Plain {}
mixin Behavior {}
class FamilyNotifier<T, Arg> {}
class FamilyLookalike extends FamilyNotifier<int, String> {}
class BuildlessStreamNotifier<T> {}
class BuildlessLookalike extends BuildlessStreamNotifier<int> {}
class AnyNotifier<StateT, ValueT> {}
class AnyLookalike extends AnyNotifier<int, int> {}
