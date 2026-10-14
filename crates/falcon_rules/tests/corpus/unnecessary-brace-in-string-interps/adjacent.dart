String rawLast(String name) => '${name}' r'\d'; /* expect: unnecessary-brace-in-string-interps */
String rawFirst(String name) => r'\d' '${name}'; /* expect: unnecessary-brace-in-string-interps */
