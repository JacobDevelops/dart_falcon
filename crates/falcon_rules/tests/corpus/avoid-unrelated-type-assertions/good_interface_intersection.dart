class A {}
class B {}
class Both extends A implements B {}
bool isB(A value) => value is B;
void main() { print(isB(Both())); }

// `List` and `Map` are open interfaces, so no hierarchy proof rules out a
// type implementing both.
bool isMap(List<int> values) => values is Map;
