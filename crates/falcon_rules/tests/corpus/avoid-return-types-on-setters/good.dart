// Setters declared without a return type — the correct form.
class A {
  int _v = 0;
  set value(int v) { _v = v; }
  int _w = 0;
  set width(int v) => _w = v;
  static set flag(bool v) {}
  external set ext(int v);
  // Comments between modifiers are not return types.
  static /* cached */ set cached(int v) {}
  set /* note */ noted(int v) {}
  static // trailing note
      set commented(int v) {}
}

mixin M {
  set data(int v) {}
}

class B {
  int getValue() => 1;
  void doThing() {}
}
