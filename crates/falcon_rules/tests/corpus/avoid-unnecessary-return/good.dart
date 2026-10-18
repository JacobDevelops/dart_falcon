void guard(bool ready) {
  if (ready) return;
  print('work');
}
int value() { return 1; }
void loop(bool ready) {
  while (ready) { return; }
}
void counted() {
  for (var i = 0; i < 3; i++) { return; }
}
void handling() {
  try { return; } finally { print('cleanup'); }
}
void selection(int value) {
  switch (value) {
    case 1: return;
    default: print('other');
  }
}
void nested(bool ready) {
  if (ready) {
    if (ready) return;
    print('work');
  }
}
void identical(bool ready) {
  if (ready) { return; } else { return; }
}
void identicalWork(bool ready) {
  if (ready) { print('work'); return; }
  else { print('work'); return; }
}
