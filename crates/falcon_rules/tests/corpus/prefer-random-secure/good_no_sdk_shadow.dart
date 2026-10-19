import 'dart:math';
import 'dart:math' as math hide Random;

class Random {
  Random();
}

final local = Random();
final explicitLocal = new Random();
final hidden = math.Random();
final explicitHidden = new math.Random();
