class Callbacks {
  final Function callback;
  Callbacks(this.callback);
}

void invoke(Callbacks callbacks) {
  callbacks /* expect: avoid-dynamic-calls */
      .callback();
}
