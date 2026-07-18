bool a(int x) => x == x; /* expect: no-self-comparisons */

bool b(int x) => x != x; /* expect: no-self-comparisons */

bool c(int x) => x < x; /* expect: no-self-comparisons */

class Foo {
  int value = 0;

  bool check() => value >= value; /* expect: no-self-comparisons */

  bool nested(List<int> a) => a[0] == a[0]; /* expect: no-self-comparisons */
}

void d(int x) {
  if (x > x) { /* expect: no-self-comparisons */
    print('never');
  }
}
