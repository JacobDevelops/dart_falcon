int literal(bool ready) {
  if (ready) { return 1; } /* expect: avoid-unnecessary-if */
  return 1;
}
String text(bool ready) {
  if (ready) return 'same'; /* expect: avoid-unnecessary-if */
  return 'same';
}
int variable(bool ready, int value) {
  if (ready) { return value; } /* expect: avoid-unnecessary-if */
  return value;
}
int grouped(bool ready) {
  if (ready) { return (2); } /* expect: avoid-unnecessary-if */
  return 2;
}
class Example {
  int method(bool ready) {
    if (ready) return 3; /* expect: avoid-unnecessary-if */
    return 3;
  }
}
