void handle() { print(1); print(2); }
void outer() {
  void local() { print(1); print(2); }
  local();
}
int arrow() => 1;
