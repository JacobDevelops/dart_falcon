void handle(bool a, bool b) {
  if (a) {}
  void helper() { if (b) { print(1); } }
  if (b) {}
}
bool sequence(bool a, bool b, bool c, bool d) => a && b && c && d;
void guarded(bool a) { if (a) {} else {} }
