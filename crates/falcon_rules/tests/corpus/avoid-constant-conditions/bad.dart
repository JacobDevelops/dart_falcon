const enabled = true;
void example(int x) {
  if (enabled == true) {} /* expect: avoid-constant-conditions */
  if (3 > 2) {} /* expect: avoid-constant-conditions */
  while (1 == 2) {} /* expect: avoid-constant-conditions */
  do {} while (1 == 2); /* expect: avoid-constant-conditions */
  for (; enabled == true; ) { break; } /* expect: avoid-constant-conditions */
  final result = enabled == true ? 1 : x; /* expect: avoid-constant-conditions */
  final list = [if (enabled == true) 1]; /* expect: avoid-constant-conditions */
  final map = {if (enabled == true) 1: 2}; /* expect: avoid-constant-conditions */
  print([result, list, map]);
}

void guards(Object value) {
  if (value case int _ when enabled == true) {} /* expect: avoid-constant-conditions */
  switch (value) {
    case int _ when enabled == true: print(value); /* expect: avoid-constant-conditions */
  }
  final result = switch (value) {
    int _ when enabled == true => 1, /* expect: avoid-constant-conditions */
    _ => 0,
  };
  print(result);
}

enum Choice { first, second }
class Settings {
  static const enabled = true;
}
void enumAndStatic() {
  if (Choice.first == Choice.second) {} /* expect: avoid-constant-conditions */
  if (Settings.enabled == true) {} /* expect: avoid-constant-conditions */
}
