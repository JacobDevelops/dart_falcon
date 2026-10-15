String concat(List<String> parts) {
  var result = '';
  for (final part in parts) {
    result += part; /* expect: use-string-buffers */
  }
  return result;
}

String whileLoop(int n) {
  String text = '';
  var i = 0;
  while (i < n) {
    text = text + '$i'; /* expect: use-string-buffers */
    i++;
  }
  return text;
}

String doLoop(int n) {
  var text = '';
  do {
    text = '$text,'; /* expect: use-string-buffers */
    n--;
  } while (n > 0);
  return text;
}

String classic(int n) {
  var s = '';
  for (var i = 0; i < n; i++) {
    {
      s += 'x'; /* expect: use-string-buffers */
    }
  }
  return s;
}

String interpolatedBraces(List<int> values) {
  var s = '';
  for (final v in values) {
    s = '${s}x$v'; /* expect: use-string-buffers */
  }
  return s;
}

class Joiner {
  Joiner(this.prefix, this.label);
  final String prefix;
  final String? label;

  // The variables' types come from the enclosing class's fields.
  String join(List<String> parts) {
    var s = prefix;
    for (final part in parts) {
      s += part; /* expect: use-string-buffers */
    }
    var t = label ?? '';
    for (final part in parts) {
      t += part; /* expect: use-string-buffers */
    }
    return s + t;
  }
}
