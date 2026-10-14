class Holder {
  void Function()? callback;
  int Function(int)? transform;
}

void ifStatement(void Function()? f) {
  if (f != null) f!(); /* expect: prefer-null-aware-method-calls */
}

void ifBlock(void Function(int)? f) {
  if (f != null) {
    f!(1); /* expect: prefer-null-aware-method-calls */
  }
}

void field(Holder holder) {
  if (holder.callback != null) holder.callback!(); /* expect: prefer-null-aware-method-calls */
}

int? conditional(int Function(int)? f) => f != null ? f!(2) : null; /* expect: prefer-null-aware-method-calls */

int? conditionalField(Holder h) =>
    h.transform != null ? h.transform!(3) : null; /* expect: prefer-null-aware-method-calls */

void spacing(void Function()? f) {
  if (f!= null) f !(); /* expect: prefer-null-aware-method-calls */
}
