library dart.math;

abstract class Random {
  external factory Random([int? seed]);
  external factory Random.secure();
  int nextInt(int max);
  double nextDouble();
  bool nextBool();
}
