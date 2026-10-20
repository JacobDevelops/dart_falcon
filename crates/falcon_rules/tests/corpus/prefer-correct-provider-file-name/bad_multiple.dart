import 'package:riverpod/riverpod.dart' as rp;
final countProvider = rp.Provider<int>((ref) => 0); /* expect: prefer-correct-provider-file-name */
final otherProvider = rp.FutureProvider<int>((ref) => 0);
