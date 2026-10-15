bool even(int x) => x.isEven;
bool other(int x) => x % 3 == 0;
bool negative(int x) => x % 2 == -1;
bool asDouble(double x) => x % 2 == 0;
bool asNum(num x) => x % 2 == 0;
bool notEqual(int x) => x % 2 != 0;
const isConstEven = 4 % 2 == 0;

class Grid {
  const Grid(this.size) : assert(size % 2 == 0);
  final int size;
  static const half = 10 % 2 == 0;
}

bool groupedModulo(int i) => (i % 2) == 0;
bool groupedModuloWithComments(int i) => (/* group */ i % 2 /* group */) == 0;
bool groupedDivisor(int i) => i % (2) == 0;
bool groupedComparisonValue(int i) => i % 2 == (0);
