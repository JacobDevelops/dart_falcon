class Box {
  final int _size = 3;
  int size() => _size; /* expect: prefer-getter-over-method */
  int otherSize() { return this._size; } /* expect: prefer-getter-over-method */
}
