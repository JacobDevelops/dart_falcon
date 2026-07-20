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
