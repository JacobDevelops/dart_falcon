import 'dart:math';
import 'dart:math' as math;
final secure = Random.secure();
final prefixedSecure = math.Random.secure();
final seeded = Random(42);
final explicitSeeded = new Random(7);
final prefixedSeeded = math.Random(1);
void shadow(dynamic Random) { Random(); }
class Other { Other(); }
final other = Other();
