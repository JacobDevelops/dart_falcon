// Modern generic function type syntax for parameters, plus plain params.
void forEach(int Function(int x) f) {}
void run(void Function() cb) {}
int apply(int Function(int, int) g) => g(1, 2);
void sortBy(bool Function(int, int) compare) {}
void plain(int a, String b) {}

class C {
  void method(void Function() fn) {}
}
