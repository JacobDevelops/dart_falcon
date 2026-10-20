import 'package:riverpod/riverpod.dart' as rp;
final countProvider = rp.FutureProvider<int>((ref) => 0); /* expect: prefer-correct-provider-file-name */
