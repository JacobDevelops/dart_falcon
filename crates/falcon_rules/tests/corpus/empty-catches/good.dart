void g1() {
  try {
    doThing();
  } catch (e) {
    print(e);
  }
}

void g2() {
  try {
    doThing();
  } catch (_) {}
}

void g3() {
  try {
    doThing();
  } catch (e) {
    // Failure is expected and intentionally ignored here.
  }
}

void g4() {
  try {
    doThing();
  } on Exception catch (e) {
    handle(e);
  }
}

void g5() {
  try {
    doThing();
  } on StateError catch (_) {}
}

void g6() {
  try {
    doThing();
  } catch (e, st) {
    print('$e $st');
  }
}
