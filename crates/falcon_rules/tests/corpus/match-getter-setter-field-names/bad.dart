class State {
  int _count = 0;
  String _name = '';
  int _total = 0;
  int _size = 0;
  int _amount = 0;
  int _quantity = 0;
  String _title = '';
  String _label = '';
  int get total => _count; /* expect: match-getter-setter-field-names */
  int get size { return this._count; } /* expect: match-getter-setter-field-names */
  set amount(int value) => _count = value; /* expect: match-getter-setter-field-names */
  set quantity(int value) { this._count = value; } /* expect: match-getter-setter-field-names */
  String get title => _name; /* expect: match-getter-setter-field-names */
  set label(String value) { _name = value; } /* expect: match-getter-setter-field-names */
}
