final callback = () {
  if (true) { /* expect: avoid-nested-if */
    final callbacks = [() {
      if (true) {}
      if (false) {}
    }];
  }
};
