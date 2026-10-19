class BannedCascadeOther { int size = 0; }
class CascadeBox {
  int size = 0;
  void copy(BannedCascadeOther other) {
    other..size = 1; /* expect: banned-usage, msg: "Avoid the receiver field." */
    other..size += 1; /* expect: banned-usage, msg: "Avoid the receiver field." */
    other..size = size; /* expect: banned-usage, msg: "Avoid the receiver field." */ /* expect: banned-usage, msg: "Avoid the enclosing field." */
  }
}
