bool a(int x) => x == x; /* expect: no_self_comparisons */

bool b(int x) => x != x; /* expect: no_self_comparisons */

bool c(int x) => x < x; /* expect: no_self_comparisons */

class Foo {
  int value = 0;

  bool check() => value >= value; /* expect: no_self_comparisons */

  bool nested(List<int> a) => a[0] == a[0]; /* expect: no_self_comparisons */
}

void d(int x) {
  if (x > x) { /* expect: no_self_comparisons */
    print('never');
  }
}
