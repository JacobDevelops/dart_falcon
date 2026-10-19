class Box { int value = 0; }
void noMutation(Box box, List<int> values) {
  final local = Box();
  local.value = 1;
  final copy = [...values];
  copy[0] = 2;
  final read = box.value;
  box = Box(); // Rebinding belongs to parameter-assignments.
  {
    final box = Box();
    box.value = read;
  }
  void nested() { final box = Box(); box.value++; }
  final callback = () { final values = [0]; values[0]++; };
  nested(); callback();
}
class Owner {
  Box box;
  Owner(this.box) { box.value = 1; }
}
const mutated = 0;
void intentional(@mutated Box box, @mutated List<int> values) {
  box.value = 1;
  values[0]++;
}
class AnnotatedSetter {
  set item(@mutated Box value) { value.value++; }
}

class Builder { String name = ''; }
class Options { final headers = <String, String>{}; }
class Item { bool selected = true; }

void closures(List<Item> items) {
  final build = (Builder b) => b..name = 'x';
  final intercept = (Options options) {
    options.headers['Authorization'] = 'token';
  };
  items.forEach((i) => i.selected = false);
  build(Builder());
  intercept(Options());
}
