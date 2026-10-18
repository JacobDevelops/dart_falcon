void read() { int x = 1; print(x); x = 2; }
void call() { Object x = Object(); x = Object(); }
void lateLocal() { late int x = 1; x = 2; }
void compound() { int x = 1; x += 2; }
void dependsOnOld() { int x = 1; x = x + 2; }
void selfAssignment() { int x = 1; x = x; }
void capture() {
  Object? x = null;
  x = () => x;
  print(x);
}
class C {
  int get value => 1;
  set value(int x) {}
  void field() { value = 2; }
}
void shadow() {
  var x = 1;
  void nested(int x) { x = 2; }
  nested(x);
}
void inferredDynamic() {
  var x = null;
  x = 1;
  x = 'a';
  print(x);
}
void inferredFromDynamic(dynamic next) {
  var x = 0;
  x = next;
  print(x);
}
void interpolatedInitializer(Object next) {
  String x = '$next';
  x = 'replacement';
  print(x);
}
void branchOverwrite(bool flag) {
  int x = 0;
  if (flag) { x = 1; } else { x = 2; }
  print(x);
}
