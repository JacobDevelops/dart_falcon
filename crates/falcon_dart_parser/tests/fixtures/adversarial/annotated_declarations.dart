// Annotation ownership remains valid through declaration dispatch and recovery.
@first
@second(1)
class C { @field int value = 0; }
@function
void f() {}
@variable
final answer = 42;
@dangling
