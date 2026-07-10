// Good: identifiers meet the minimum length (3), plus dcl's scope exemptions.
// dcl only checks variable declarations, getter/setter names and enum
// constants — never parameters, catch clauses, for-each variables, or plain
// method/function names.

void example() {
  var count = compute();
  final result = count + 1;
  print(result);
}

class Processor {
  String path = '';
  int id = 0; // `id` is in the exceptions list
  int _id = 0; // leading underscore stripped -> `id` -> exempt

  // Method name (`at`) and parameters (`i`, `j`) are out of scope.
  int at(int i, int j) => i + j;

  // Getter name is >= 3 characters.
  String get label => path;
}

// Parameters, catch clauses and for-each variables are never checked, so short
// names here are fine.
void shortLived(int a, int b) {
  try {
    print(a + b);
  } catch (e) {
    print(e);
  }
  for (final x in [1, 2, 3]) {
    print(x);
  }
}

enum Color { red, green, blue }
