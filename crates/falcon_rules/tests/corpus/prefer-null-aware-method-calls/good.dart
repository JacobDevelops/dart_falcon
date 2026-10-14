class Holder {
  void Function()? callback;
}

void nullAware(void Function()? f) {
  f?.call();
}

void withElse(void Function()? f) {
  if (f != null) {
    f!();
  } else {
    print('none');
  }
}

void twoStatements(void Function()? f) {
  if (f != null) {
    f!();
    print('done');
  }
}

void differentTarget(void Function()? f, void Function()? g) {
  if (f != null) g!();
}

void reversed(void Function()? f) {
  if (null != f) f!();
}

int? notNullElse(int Function()? f) => f != null ? f!() : 0;

void method(Holder h) {
  if (h.callback != null) h.callback?.call();
}
