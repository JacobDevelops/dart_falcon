String rawLast(String value) => 'x' r'\d' + value;
String rawFirst(String value) => r'\d' 'x' + value;
String joined(String value) => 'x' 'y' + value; /* expect: prefer-interpolation-to-compose-strings */
