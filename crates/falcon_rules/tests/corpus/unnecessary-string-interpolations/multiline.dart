String plain(String name) => /* expect: unnecessary-string-interpolations */ '''
$name''';
String escapedLead(String name) => /* expect: unnecessary-string-interpolations */ '''\
$name''';
String escapedSpace(String name) => /* expect: unnecessary-string-interpolations */ '''\ 
$name''';
String keptSlash(String name) => '''\\
$name''';
String adjacent(String name) => '''
$name''' '';
