import 'package:riverpod/riverpod.dart' as rp;
class CounterNotifier extends rp.Notifier<int> {} /* expect: prefer-correct-notifier-file-name */
class OtherNotifier extends rp.AsyncNotifier<int> {}
