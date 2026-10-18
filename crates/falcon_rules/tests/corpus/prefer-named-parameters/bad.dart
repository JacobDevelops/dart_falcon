void configure(int width, int height, String label) {} /* expect: prefer-named-parameters */
class Box {
  Box(int width, int height, String label); /* expect: prefer-named-parameters */
  void resize(int width, int height, bool animate) {} /* expect: prefer-named-parameters */
}
