@deprecated
int oldTop = 0;
class Host {
  @deprecated
  static int count = 0;
  @deprecated
  int get old => 0;
}
extension on Host {
  static int count = 1;
  int get old => 1;
  int get oldTop => 1;
  void uses() {
    count;
    old;
    oldTop;
  }
}
