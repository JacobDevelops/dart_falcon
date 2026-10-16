String joined(String name) => '${name}' '';
String rawLast(String name) => '${name}' r'';
String rawFirst(String name) => r'' '${name}';
String adjacentText(String name) => 'prefix' '$name' 'suffix';
String standalone(String name) => '$name'; /* expect: unnecessary-string-interpolations */
