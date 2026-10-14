String joined(String name) => '${name}' ''; /* expect: unnecessary-string-interpolations */
String rawLast(String name) => '${name}' r''; /* expect: unnecessary-string-interpolations */
String rawFirst(String name) => r'' '${name}'; /* expect: unnecessary-string-interpolations */
