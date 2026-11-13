class Custom { int get $1 => 1; }
void read(({int x, String y}) record, Custom custom) {
  print(record.x);
  print(record.y);
  print(custom.$1);
}
