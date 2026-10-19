import 'dart:math' show Random;
import 'dart:math' as math show Random;

final first = Random(); /* expect: prefer-random-secure */
final second = new Random(); /* expect: prefer-random-secure */
final third = math.Random(); /* expect: prefer-random-secure */
final fourth = new math.Random(); /* expect: prefer-random-secure */
final explicitNew = Random.new(); /* expect: prefer-random-secure */
final prefixedNew = math.Random.new(); /* expect: prefer-random-secure */
