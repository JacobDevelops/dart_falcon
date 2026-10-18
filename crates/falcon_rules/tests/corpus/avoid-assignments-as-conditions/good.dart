bool invoke(bool Function() callback) => callback();

void conditions(bool input, bool other, bool? nullable) {
  var flag = false;
  var count = 0;
  flag = input;
  if (flag == other) { flag = other; }
  while (flag) { flag = false; }
  do { flag = input; } while (flag);
  for (flag = input; flag; flag = false) { count++; }
  final branch = input ? (flag = other) : false;
  final list = [if (input) flag = other];
  final map = {if (input) 'value': flag = other};
  final generated = [for (flag = input; flag; flag = false) count];
  final keyed = {for (flag = input; flag; flag = false) 'value': count};
  if (() { flag = input; return flag; }()) {}
  while ((() => flag = input)()) { break; }
  if (invoke(() => flag = input)) {}
  final ternary = (() => flag = input)() ? 1 : 2;
  final closures = [if ((() => flag = input)()) 1];
  if ((++count) > 2) {}
  assert(flag = input);
  nullable ??= other;
  switch (input) {
    case true when (() => flag = other)():
      flag = false;
      break;
    default:
      break;
  }
  final switched = switch (input) {
    true when input => flag = other,
    _ => false,
  };
}
