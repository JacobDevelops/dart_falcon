class Box { int value = 0; Box? child; }
void mutate(Box box, List<int> values, Map<String, int> map) {
  box.value = 1; /* expect: avoid-mutating-parameters */
  box.value += 1; /* expect: avoid-mutating-parameters */
  ++box.value; /* expect: avoid-mutating-parameters */
  box.value--; /* expect: avoid-mutating-parameters */
  values[0] = 2; /* expect: avoid-mutating-parameters */
  map['key'] ??= 3; /* expect: avoid-mutating-parameters */
  box.child!.value = 2; /* expect: avoid-mutating-parameters */
  box..value = 4; /* expect: avoid-mutating-parameters */
  values..[0] = 5; /* expect: avoid-mutating-parameters */
  void nested() { box.value = 6; /* expect: avoid-mutating-parameters */ }
  final closure = () { values[0]++; /* expect: avoid-mutating-parameters */ };
  nested(); closure();
}
class Holder {
  set item(Box value) { value.value = 1; /* expect: avoid-mutating-parameters */ }
}

class Mutator {
  Mutator(Box box) {
    box.value = 1; /* expect: avoid-mutating-parameters */
  }

  void update(Box box) {
    box.value = 2; /* expect: avoid-mutating-parameters */
  }

  void operator []=(int index, Box box) {
    box.value = index; /* expect: avoid-mutating-parameters */
  }
}

void outer() {
  void local(Box box) {
    box.value = 3; /* expect: avoid-mutating-parameters */
  }
  local(Box());
}
