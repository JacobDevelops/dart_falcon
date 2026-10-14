bool even(int x) => x.isEven;
bool other(int x) => x % 3 == 0;
bool notTwo(int x) => x % 2 == 2;
bool asDouble(double x) => x % 2 == 0;
bool asNum(num x) => x % 2 == 0;
bool notEqual(int x) => x % 2 != 0;
const isConstEven = 4 % 2 == 0;

class Grid {
  const Grid(this.size) : assert(size % 2 == 0);
  final int size;
  static const half = 10 % 2 == 0;
}

void defaults([bool flag = 3 % 2 == 1]) {}
