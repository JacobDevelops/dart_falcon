class Context {
  void read() {}
}

typedef Callback = void Function(Context);

void fallback(Callback? callback) {
  dynamic executor = callback ?? (ctx) => ctx.read();
  final Map<String, dynamic> values = {
    'run': callback ?? (ctx) => ctx.read(),
  };
}
