class Accessors {
  @deprecated
  int get value => 0;
  set value(int value) {}
}
void writeOnly(Accessors object) { object.value = 1; }
void genericShadow<Legacy>(Legacy value) { print(value); }
