// @dart = 3.13
int get x => 0;
int get value => 0;

class PrimaryField(final int x) {
  void check() {
    x; /* expect: unnecessary-statements */
  }
}

class PrimaryVarField(var int value) {
  void check() {
    value; /* expect: unnecessary-statements */
  }
}
