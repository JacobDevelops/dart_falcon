// An `@pragma('vm:entry-point')` function makes the library executable even
// without `main`.

@pragma('vm:entry-point')
void start() {
  reachableFromEntryPoint();
}

void reachableFromEntryPoint() {}

void neverCalled() {} /* expect: unreachable-from-main */

class NeverConstructed {} /* expect: unreachable-from-main */
