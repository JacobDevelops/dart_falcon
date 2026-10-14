@deprecated
const oldMessage = 'old';

class Marker {
  final String value;
  const Marker(this.value);
}

@Marker(oldMessage) /* expect: deprecated-member-use-from-same-package */
void marked() {}

class NamedMarker {
  const NamedMarker({@deprecated String? oldOption, String? option});
}

@NamedMarker(oldOption: 'old') /* expect: deprecated-member-use-from-same-package */
void namedMarked() {}

@deprecated
@Marker(oldMessage)
void obsoleteMarked() {}

void localShadow() {
  const oldMessage = 'fresh';
  @Marker(oldMessage)
  const value = 0;
  print(value);
}
