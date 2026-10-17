void save() async {} /* expect: avoid-void-async */

void arrow() async => print('x'); /* expect: avoid-void-async */

class Store {
  void flush() async {} /* expect: avoid-void-async */

  static void reset() async {} /* expect: avoid-void-async */

  void get refresh async {} /* expect: avoid-void-async */
}

void outer() {
  void inner() async {} /* expect: avoid-void-async */
  inner();
}

mixin Loader {
  void load() async {} /* expect: avoid-void-async */
}

void main() async {
  void callback() async {} /* expect: avoid-void-async */
  [callback].forEach((run) => run());
}
