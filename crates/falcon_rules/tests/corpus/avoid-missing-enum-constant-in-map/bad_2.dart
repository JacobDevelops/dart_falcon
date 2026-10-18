enum Color { red, green, blue }

final partial = <Color, int>{...{Color.red: 1}, Color.green: 2}; /* expect: avoid-missing-enum-constant-in-map */
