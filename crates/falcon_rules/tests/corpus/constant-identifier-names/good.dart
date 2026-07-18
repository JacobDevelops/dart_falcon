// lowerCamelCase constant identifiers.

const maxValue = 100;

const pi = 3.14;

const httpPort = 80;

enum E { first, second }

class C {
  static const defaultSize = 10;
}

void f() {
  const localConst = 1;
  print(localConst);
}

void g() {
  for (const forLimit = 3; forLimit > 0;) {
    print(forLimit);
  }
}
