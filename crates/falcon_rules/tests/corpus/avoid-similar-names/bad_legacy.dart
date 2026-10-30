// The declared name is nested in LegacyTypedef, but belongs to program scope.
typedef int Response(int value);
typedef int Responses(int value); /* expect: avoid-similar-names */
