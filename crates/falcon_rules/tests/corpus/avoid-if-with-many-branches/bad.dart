void handle(int x) {
  if (x == 0) { /* expect: avoid-if-with-many-branches */
    print(0);
  } else if (x == 1) { print(1); } else { print(2); }
}
