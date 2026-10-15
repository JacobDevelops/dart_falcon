class Options {
  Options();

  // The factory's self return type supplies fold's accumulator context.
  factory Options.combine(Iterable<Options> options) =>
      options.fold(Options(), (p, o) => p.merge(o));

  Options merge(Options? other) => this;
}
