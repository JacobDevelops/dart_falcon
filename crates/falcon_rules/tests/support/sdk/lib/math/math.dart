library dart.math;

abstract class Random {
  factory Random([int? seed]);
  factory Random.secure();
  int nextInt(int max);
  double nextDouble();
  bool nextBool();
}
