// @dart = 3.13

@deprecated
class Unnamed(); /* expect: deprecated-consistency */

@deprecated
class Named.named(); /* expect: deprecated-consistency */

@deprecated
extension type Representation(int value) {} /* expect: deprecated-consistency */

class Header(this.value) { /* expect: deprecated-consistency */
  @deprecated
  int value;
}

class Reverse(@deprecated this.value) {
  int value; /* expect: deprecated-consistency */
}

@deprecated
class Implemented() { /* expect: deprecated-consistency */
  this {}
}
