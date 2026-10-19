import 'dart:math';
import 'dart:math' as math;
final first = Random(); /* expect: prefer-random-secure */
final second = new Random(); /* expect: prefer-random-secure */
final third = math.Random(); /* expect: prefer-random-secure */
final fourth = new math.Random(); /* expect: prefer-random-secure */
Random create() => Random(); /* expect: prefer-random-secure */
final explicitNew = Random.new(); /* expect: prefer-random-secure */
