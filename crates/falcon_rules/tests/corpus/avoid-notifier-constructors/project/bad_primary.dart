// @dart = 3.13
import 'package:riverpod/riverpod.dart' as rp;

class PrimaryFinal(final int seed) extends rp.Notifier<int> {} /* expect: avoid-notifier-constructors */
class PrimaryVar(var int seed) extends rp.Notifier<int> {} /* expect: avoid-notifier-constructors */
