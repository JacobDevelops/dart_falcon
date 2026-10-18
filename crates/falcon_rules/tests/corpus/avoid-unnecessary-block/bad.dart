void expressions() {
  { print('nested'); } /* expect: avoid-unnecessary-block */
}

void empty() {
  {} /* expect: avoid-unnecessary-block */
}

int returning() {
  { return 1; } /* expect: avoid-unnecessary-block */
}

void multiple() {
  { print('first'); print('second'); } /* expect: avoid-unnecessary-block */
}

void conditional(bool ready) {
  { if (ready) { print('ready'); } } /* expect: avoid-unnecessary-block */
}

void scopedChild() {
  { /* expect: avoid-unnecessary-block */
    { final value = 1; print(value); }
  }
}
