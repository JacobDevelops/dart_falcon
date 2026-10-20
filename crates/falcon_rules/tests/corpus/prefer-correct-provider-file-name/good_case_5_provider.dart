import 'package:hooks_riverpod/hooks_riverpod.dart' as rp;
final countProvider = rp.AutoDisposeProvider<int>((ref) => 0);
