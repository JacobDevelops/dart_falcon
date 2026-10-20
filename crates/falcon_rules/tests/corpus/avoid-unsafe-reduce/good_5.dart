import 'dart:math' as math;

int add(int a, int b) => a + b;

double average(List<int> xs) => xs.isEmpty ? 0 : xs.reduce(add) / xs.length;
String display(List<int> xs) => xs.isEmpty ? '' : xs.reduce(math.max).toString();

int assign(List<int> xs) {
  var sum = 0;
  if (xs.isNotEmpty) {
    sum = xs.reduce(add);
  }
  return sum;
}

int localFunction(List<int> xs) {
  int combine(int a, int b) => a + b;
  return xs.isEmpty ? 0 : xs.reduce(combine);
}

class Combiner {
  int combine(int a, int b) => a + b;
  int implicit(List<int> xs) => xs.isEmpty ? 0 : xs.reduce(combine);
  int explicit(List<int> xs) => xs.isEmpty ? 0 : xs.reduce(this.combine);
}
