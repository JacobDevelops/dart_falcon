int delegates(int Function() delegates) => delegates();
int other() => 1;
int wrapper() => other();
class Valid {
  Valid? next;
  int run() => next!.run();
  int shadow(int Function() shadow) => shadow();
  int get recursiveGetter => recursiveGetter; // Covered by recursive-getters.
}
void nested() { void closure() { print('not recursive'); } closure(); }
int iterative(int n) { var result = 0; while (n > 0) { result += n; n--; } return result; }
void shadowed() {
  final shadowed = () => 1;
  print(shadowed());
}
void captured() { final callback = () => captured(); print(callback); }
