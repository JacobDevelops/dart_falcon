const selected = 2;
void example() {
  switch (selected) { /* expect: avoid-constant-switches */
    case 2: print('two');
    default: print('other');
  }
  final value = switch ('yes') { /* expect: avoid-constant-switches */
    'yes' => 1,
    _ => 0,
  };
  switch (1 + 2) { /* expect: avoid-constant-switches */
    case 3: print(value);
  }
}

enum Choice { first, second }
class Settings {
  static const selected = 2;
}
void enumAndStatic() {
  switch (Choice.first) { /* expect: avoid-constant-switches */
    case Choice.first: break;
    case Choice.second: break;
  }
  switch (Settings.selected) { /* expect: avoid-constant-switches */
    case 2: break;
    default: break;
  }
  final value = switch (Choice.second) { /* expect: avoid-constant-switches */
    Choice.first => 1,
    Choice.second => 2,
  };
  print(value);
}
