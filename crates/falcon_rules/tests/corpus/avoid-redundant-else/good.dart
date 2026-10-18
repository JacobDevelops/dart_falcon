void work(bool ready) {
  if (ready) { print('first'); } else { print('second'); }
}
void trailing(bool ready) {
  if (ready) { return; } else { print('second'); }
}
void conditional(bool ready, bool other) {
  if (ready) { if (other) return; } else { print('second'); }
  print('after');
}
int identical(bool ready) {
  if (ready) { return 1; } else { return 1; }
}
void noElse(bool ready) {
  if (ready) return;
  print('after');
}
void loop(bool ready) {
  while (ready) {
    if (ready) { break; } else { print('second'); }
  }
}
void chain(bool first, bool second) {
  if (first) { print('first'); }
  else if (second) { return; }
  else { print('third'); }
  print('after');
}
int nestedUnbraced(bool first, bool second) {
  if (first) if (second) return 1; else return 2;
  return 3;
}
void loopUnbraced(bool ready) {
  while (ready) if (ready) return; else print('step');
}
