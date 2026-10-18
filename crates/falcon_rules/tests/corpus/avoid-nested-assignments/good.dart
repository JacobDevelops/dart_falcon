void f(dynamic x) {
  x = 1;
  x += 2;
  (x = 3);
  for (x = 0; x < 5; x = x + 1) {}
  final closure = () { x = 4; };
  x ??= 6;
  final arrow = () => x = 7;
  final lazy = () => x ??= 8;
  x = (x = 9);
  if ((x = true)) {}
  while ((x = readLine()) != null) {}
  final owner = Owner();
  owner..value = x = 10;
  consume(x = x = 11);
}

class Owner {
  dynamic value;
  set name(String input) => value = input;
  dynamic get cached => value ??= Owner();
}
