class CascadeNode {
  CascadeNode? parent;
  CascadeNode? operator [](int index) => parent;
  void Function()? callback;

  void clear() {}
  void go(CascadeNode node) {}
}

void cascadeAssertions(CascadeNode c) {
  c..clear()..parent!.go(c);
  c.go; /* expect: unnecessary-statements */
  c..go; /* expect: unnecessary-statements */
  c..parent![0]!.go(c);
  c..callback!();
}
