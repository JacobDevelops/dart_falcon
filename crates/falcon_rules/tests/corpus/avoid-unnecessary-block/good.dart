void variables() {
  final value = 1;
  { final value = 2; print(value); }
  print(value);
}

void patterns() {
  { var (first, second) = (1, 2); print(first + second); }
}

void functions() {
  { void local() {} local(); }
}

void controlBodies(bool ready) {
  if (ready) { print('ready'); } else { print('waiting'); }
  while (ready) { break; }
  for (var i = 0; i < 2; i++) { print(i); }
  do { print('once'); } while (false);
}

void exceptionBodies() {
  try { print('try'); } catch (error) { print(error); } finally { print('finally'); }
}

void labels() {
  exit: { print('labeled'); break exit; }
}

void laterDeclaration() {
  { print('before'); final value = 1; print(value); }
}
