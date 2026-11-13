/** Class documentation. */ /* expect: slash-for-doc-comments */
class Surface {
  /** Field documentation. */ /* expect: slash-for-doc-comments */
  int value = 0;
  /** Constructor documentation. */ /* expect: slash-for-doc-comments */
  Surface();
  /** Method documentation. */ /* expect: slash-for-doc-comments */
  void method() {}
}
void outer() {
  /** Local function documentation. */ /* expect: slash-for-doc-comments */
  void inner() {}
  inner();
}
