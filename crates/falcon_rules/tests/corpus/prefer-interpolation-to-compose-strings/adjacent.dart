String rawLast(String value) => 'x' r'\d' + value; /* expect: prefer-interpolation-to-compose-strings */
String rawFirst(String value) => r'\d' 'x' + value; /* expect: prefer-interpolation-to-compose-strings */
String joined(String value) => 'x' 'y' + value; /* expect: prefer-interpolation-to-compose-strings */
