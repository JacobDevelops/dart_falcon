String rawLast = '\q' r'\d'; /* expect: unnecessary-string-escapes */
String rawFirst = r'\d' '\q'; /* expect: unnecessary-string-escapes */
