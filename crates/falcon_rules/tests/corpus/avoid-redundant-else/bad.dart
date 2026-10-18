int returning(bool ready) {
  if (ready) { return 1; }
  else { print('other'); } /* expect: avoid-redundant-else */
  return 2;
}
void early(bool ready) {
  if (ready) { return; }
  else { print('other'); } /* expect: avoid-redundant-else */
  print('after');
}
int unbraced(bool ready) {
  if (ready) return 1;
  else return 2; /* expect: avoid-redundant-else */
}
void throwing(bool ready) {
  if (ready) { throw StateError('stop'); }
  else { print('other'); } /* expect: avoid-redundant-else */
}
void rethrowing(bool ready) {
  try { throw StateError('stop'); } catch (error) {
    if (ready) { rethrow; }
    else { print(error); } /* expect: avoid-redundant-else */
  }
}
int comments(bool ready) {
  if (ready) { return 1; /* explanation */ }
  else { return 1; } /* expect: avoid-redundant-else */
}
int exitingChain(bool first, bool second) {
  if (first) return 1;
  else if (second) return 2; /* expect: avoid-redundant-else */
  else return 3;
}
