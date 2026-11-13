class A {}
class B {}
class Both extends A implements B {}
bool isB(A value) => value is B;
void main() { print(isB(Both())); }
