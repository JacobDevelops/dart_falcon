bool a(int x, int y) => x == y;

bool b(int x) => x == x + 1;

bool c(int x) => x < x + 1;

class Foo {
  int a = 0;
  int b = 0;

  bool check() => a == b;

  bool indexed(List<int> l) => l[0] == l[1];
}

void d(int x, int y) {
  if (x > y) {
    print('maybe');
  }
}

bool e(int x) => -x == x;

// Side-effecting calls evaluate to different values, so identical text is
// not a self-comparison.
bool popEqual(List<int> l) => l.removeLast() == l.removeLast();

bool nowEqual() => DateTime.now() == DateTime.now();

int nextId = 0;
int gen() => nextId++;
bool genEqual() => gen() == gen();

// Prefix ++/-- mutate the operand, so the two sides differ.
bool preIncrement(int x) => ++x == ++x;

bool preDecrement(int x) => --x == --x;

int get changing => nextId++;
bool getterEqual() => changing == changing;
bool sameIndex(List<int> values) => values[0] == values[0];

class WeirdEquality {
  @override
  bool operator ==(Object other) => false;

  bool compare() => this == this;
}
