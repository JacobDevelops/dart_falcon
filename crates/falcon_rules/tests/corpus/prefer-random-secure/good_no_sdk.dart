import 'dart:math' show Random;
import 'dart:math' as math show Random;

final secure = Random.secure();
final prefixedSecure = math.Random.secure();
final seeded = Random(42);
final explicitSeeded = new Random(7);
final prefixedSeeded = math.Random(1);

void shadow(dynamic Random, dynamic math) {
  Random();
  math.Random();
}
