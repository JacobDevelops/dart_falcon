import 'package:flutter_riverpod/flutter_riverpod.dart' as rp;
final countProvider = rp.StateProvider<int>((ref) => 0); /* expect: prefer-correct-provider-file-name */
