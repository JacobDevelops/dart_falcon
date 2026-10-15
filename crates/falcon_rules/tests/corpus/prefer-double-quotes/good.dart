import "dart:core";

void use(Object o) {}

void main() {
  var name = "Ann";
  use("double");
  use('say "hi"');
  use('say \"hi\"');
  use('Hello ${name.isEmpty ? "x" : "y"}');
  use("outer ${'nested'}");
  use('contains "$name"');
  use(r"raw");
}

void patternStrings(Object value) {
  switch (value) {
    case "a${'b'}":
    case "a${'b ${'c'}'}":
      break;
  }
}
