var counter = 0; /* expect: avoid-top-level-member-access */
class A { int counter = 5; int get g => counter; void m() { print(counter); } }
void g() { final r = switch (1) { final counter => counter }; print(r); }
extension E on int { int get counter => 1; int m() => counter; }
mixin M { int counter = 0; int m() => counter; }
enum En { a; final int counter = 0; int m() => counter; }
class P(final int counter) { int m() => counter; }
void readGlobal() { print(counter); } /* expect: avoid-top-level-member-access */
