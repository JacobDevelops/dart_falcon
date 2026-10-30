class AcceptedTypes {
  final Set<Type> types;
  const AcceptedTypes(this.types);
}

void local(@AcceptedTypes({String}) Object value) {}
void example() {
  local(1);
}
