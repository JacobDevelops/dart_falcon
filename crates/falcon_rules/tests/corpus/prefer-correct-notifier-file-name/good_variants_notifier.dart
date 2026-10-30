import 'package:riverpod/riverpod.dart' as rp;
class FamilyCounterNotifier extends rp.FamilyNotifier<int, String> {}
class StreamCounterNotifier extends rp.BuildlessStreamNotifier<int> {}
class AnyCounterNotifier extends rp.AnyNotifier<int, int> {}
