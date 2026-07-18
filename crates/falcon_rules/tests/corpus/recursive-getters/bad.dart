class A {
  int get x => x; /* expect: recursive-getters */

  int get y => this.y; /* expect: recursive-getters */

  String get label {
    return label; /* expect: recursive-getters */
  }

  double get z {
    return this.z; /* expect: recursive-getters */
  }

  num get w => this.w; /* expect: recursive-getters */
}

int get topLevel => topLevel; /* expect: recursive-getters */
