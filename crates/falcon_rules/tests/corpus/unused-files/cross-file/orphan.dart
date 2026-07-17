/* expect: unused-files */
// Nothing imports/exports this file and it has no main(): it is dead code.
void orphanHelper() {
  print('orphan');
}
