int a((int, int) pair) {
  final (first, second) = pair;
  return pair.$1; /* expect: use-existing-destructuring */
}
int b((int, int) pair) {
  final (first, second) = pair;
  return pair.$2; /* expect: use-existing-destructuring */
}
String c(({String name}) person) {
  final (:name) = person;
  return person.name; /* expect: use-existing-destructuring */
}
int d(({int count}) record) {
  final (count: total) = record;
  return record.count; /* expect: use-existing-destructuring */
}
int e() {
  final pair = (1, 2);
  var (first, second) = pair;
  print(pair.$1); /* expect: use-existing-destructuring */
  return first;
}
int extendPositional((int, int) pair) {
  final (first, _) = pair;
  return pair.$2; /* expect: use-existing-destructuring */
}
int extendNamed(({int first, int second}) pair) {
  final (:first) = pair;
  return pair.second; /* expect: use-existing-destructuring */
}
