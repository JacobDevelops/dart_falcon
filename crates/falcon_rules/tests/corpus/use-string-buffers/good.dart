String buffer(List<String> parts) {
  final buffer = StringBuffer();
  for (final part in parts) {
    buffer.write(part);
  }
  return buffer.toString();
}

String declaredInside(List<String> parts) {
  var last = '';
  for (final part in parts) {
    var s = '';
    s += part;
    last = s;
  }
  return last;
}

int numbers(List<int> values) {
  var total = 0;
  for (final v in values) {
    total += v;
  }
  return total;
}

String suffix(List<String> parts) {
  var s = '';
  for (final part in parts) {
    s = part + s;
  }
  return s;
}

String nested(List<String> parts) {
  var s = '';
  for (final part in parts) {
    if (part.isNotEmpty) {
      s += part;
    }
  }
  return s;
}

String outside(String s) {
  s += '!';
  return s;
}

String prefixText(List<String> parts) {
  var s = '';
  for (final part in parts) {
    s = 'x$s';
  }
  return s;
}
