void outer() {
  void helper() { /* expect: max-statements */
    print(1); print(2); print(3);
  }
  helper();
}
