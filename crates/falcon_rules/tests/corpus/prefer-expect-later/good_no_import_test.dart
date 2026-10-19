void test(String name, dynamic body) {}
var global = 0;
void main() { test("local", () {}); }
