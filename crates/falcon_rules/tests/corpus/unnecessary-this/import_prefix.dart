import 'dart:math' as prefixName;

class PrefixBase {
  int get prefixName => 1;
}

class InheritedPrefix extends PrefixBase {
  int read() => this.prefixName;
}

class DeclaredPrefix {
  int get prefixName => 1;
  int read() => this.prefixName; /* expect: unnecessary-this */
}
