class AllowedCascadeOther { int size = 0; }
class CascadeBox {
  int size = 0;
  void copy(AllowedCascadeOther other) {
    other..size = 1;
    other..size += 1;
  }
}
