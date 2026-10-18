class Modern {}
Modern? value;
List<Modern> values = [];
// Unknown types cannot establish a forbidden declaration identity.
Unknown? unknown;

class Legacy {
  const Legacy();
}
class Box<Legacy> {
  Legacy? value;
}
void generic<Legacy>(Legacy value) {
  Legacy? local;
}
void outer() {
  void nested<Legacy>(Legacy value) {}
}
typedef Callback = void Function<Legacy>(Legacy);
void constructors() {
  Legacy();
  new Legacy();
  const Legacy();
}
