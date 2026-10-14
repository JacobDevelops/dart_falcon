enum Choice {
  @Deprecated('Choose current')
  old,
  current;
}

void useChoice() {
  print(Choice.old); /* expect: deprecated-member-use-from-same-package */
  print(Choice.current);
}
