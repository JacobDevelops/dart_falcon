class C {
  int x = 0;
  int y = 0;

  C(int x, int y) {
    this.x = x;
    this.y = y;
  }

  // `this.x` is a property access, not an identifier.
  void reassign() {
    this.x = this.x;
  }

  void compound(int z) {
    z += z;
    z ??= z;
  }

  void different(int a, int b) {
    a = b;
  }
}

class Holder {
  int value = 0;
  Holder? next;
}

void nullAware(Holder holder) {
  holder.next?.value = holder.next?.value ?? 0;
}

void differentPrefix(Holder a, Holder b) {
  a.value = b.value;
}

void parenthesized(int a) {
  a = (a);
}

void cascade(Holder holder) {
  final value = holder.value;
  holder..value = value;
}

void deeper(Holder holder) {
  holder.next!.value = holder.next!.value;
}
