void main() {
  bool valid() => true; /* expect: boolean-prefixes */
  bool _ready() => false; /* expect: boolean-prefixes */
  print([valid(), _ready()]);
}
