class Align {
  const Align({Object? alignment, Object? child, double? widthFactor});
  const Align.named();
}

class Alignment {
  const Alignment(Object x, Object y);
  static const center = Alignment(1, 1);
}

void unrelated() {
  const Align();
  Align(alignment: Alignment.center);
  new Align(alignment: const Alignment(0, 0));
  const Align.named();
}
