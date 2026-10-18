class Legacy {}
typedef Alias = Legacy; /* expect: avoid-banned-types */
Legacy? value; /* expect: avoid-banned-types */
List<Legacy> values = []; /* expect: avoid-banned-types */
Alias? alias; /* expect: avoid-banned-types */
