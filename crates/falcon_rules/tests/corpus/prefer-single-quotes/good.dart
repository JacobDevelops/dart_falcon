import 'dart:core';

void use(Object o) {}

void main() {
  var name = 'Ann';
  use('single');
  use("it's");
  use("it\'s escaped");
  use("Hello ${name.isEmpty ? 'x' : 'y'}");
  use('outer ${"nested"}');
  use('a ${"b ${"c"}"} d ${"e"}');
  use("contains '$name'");
  use(r'raw');
}

void patternStrings(Object value) {
  switch (value) {
    case 'a${"b"}':
    case 'a${"b ${"c"}"}':
      break;
  }
}
