void repeat(bool stop) {
  for (var i = 0; i < 10; i++) { if (stop) break; }
  while (stop) { if (stop) continue; break; }
  for (var i = 0; i < 3; i++) { print(i); }
  do { if (stop) break; } while (stop);
  outer: for (;;) { break outer; }
  for (var i = 0; i < 3; i++) {
    switch (i) { case 0: break; default: print(i); }
  }
  while (stop) {
    try { break; } finally { if (stop) continue; }
  }
}
