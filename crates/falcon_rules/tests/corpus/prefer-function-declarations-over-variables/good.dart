// Proper local function declarations, and reassignable var bindings.
void f() {
  void greet() { print('hi'); }
  int add(int a, int b) => a + b;
  var handler = () => 1;
  handler = () => 2;
  final count = 3;
  final name = 'x';
  final list = <int>[];
  print(add(1, 2) + count + name.length + list.length + handler());
  greet();
}
