void f1() {
  try {
    doThing();
  } catch (e) {} /* expect: empty-catches */
}

void f2() {
  try {
    doThing();
  } on Exception catch (e) {} /* expect: empty-catches */
}

void f3() {
  try {
    doThing();
  } catch (e, st) {} /* expect: empty-catches */
}

void f4() {
  try {
    doThing();
  } on StateError {} /* expect: empty-catches */
}

void f5() {
  try {
    doThing();
  } catch (error) {} /* expect: empty-catches */
}

void f6() {
  try {
    doThing();
  } catch (e) {
  } /* expect: empty-catches */
}
