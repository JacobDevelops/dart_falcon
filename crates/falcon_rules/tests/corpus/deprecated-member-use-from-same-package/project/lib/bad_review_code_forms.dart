@deprecated
int oldIndex = 0;
class IndexBox {
  @deprecated
  int index = 0;
  @deprecated
  int operator [](int i) => 0;
  @deprecated
  void operator []=(int i, int value) {}
}
class Node {
  int counter = 0;
}
class Owner {
  @deprecated
  final List<int> values = [0];
  @deprecated
  final Node node = Node();
}
class OldBase {
  @deprecated
  int get value => 0;
  @deprecated
  set value(int v) {}
}
mixin FreshMixin {
  int get value => 1;
  set value(int v) {}
}
class FreshSuper extends OldBase with FreshMixin {
  void uses() { super.value; super.value = 1; }
}
class FreshBase {
  int get value => 1;
  set value(int v) {}
}
mixin OldMixin {
  @deprecated
  int get value => 0;
  @deprecated
  set value(int v) {}
}
class OldSuper extends FreshBase with OldMixin {
  int get value => 2;
  set value(int v) {}
  void uses() { super.value; super.value = 1; } /* expect: deprecated-member-use-from-same-package */ /* expect: deprecated-member-use-from-same-package */
}
@deprecated
int oldTop = 0;
int scopedCounter = 0;
class Host {
  @deprecated
  int get old => 0;
  @deprecated
  static int scopedCounter = 0;
}
extension OwnForms on Host {
  int get old => 1;
  int get oldTop => 1;
  @deprecated
  static void oldStatic() {}
  static void oldPositional([@deprecated int? value]) {}
  static void oldNamed({@deprecated int? value}) {}
  void uses() { old; oldTop; scopedCounter; oldStatic(); } /* expect: deprecated-member-use-from-same-package */
}
void uses(IndexBox box, Owner owner, Host host) {
  box..[oldIndex] = 1; /* expect: deprecated-member-use-from-same-package */ /* expect: deprecated-member-use-from-same-package */
  box..[oldIndex] += 1; /* expect: deprecated-member-use-from-same-package */ /* expect: deprecated-member-use-from-same-package */ /* expect: deprecated-member-use-from-same-package */
  box..[(oldIndex)] = 1; /* expect: deprecated-member-use-from-same-package */ /* expect: deprecated-member-use-from-same-package */
  box..[((oldIndex))] += 1; /* expect: deprecated-member-use-from-same-package */ /* expect: deprecated-member-use-from-same-package */ /* expect: deprecated-member-use-from-same-package */
  box..[(oldIndex)] ??= 1; /* expect: deprecated-member-use-from-same-package */ /* expect: deprecated-member-use-from-same-package */ /* expect: deprecated-member-use-from-same-package */
  box..[0].isEven; /* expect: deprecated-member-use-from-same-package */
  owner.values[0] = 1; /* expect: deprecated-member-use-from-same-package */
  owner.node.counter++; /* expect: deprecated-member-use-from-same-package */
  OwnForms.oldStatic(); /* expect: deprecated-member-use-from-same-package */
  OwnForms.oldPositional(1); /* expect: deprecated-member-use-from-same-package */
  OwnForms.oldNamed(value: 1); /* expect: deprecated-member-use-from-same-package */
  OwnForms.oldPositional();
  OwnForms.oldNamed();
  var index = 0;
  box..[index] = 1; /* expect: deprecated-member-use-from-same-package */
}
