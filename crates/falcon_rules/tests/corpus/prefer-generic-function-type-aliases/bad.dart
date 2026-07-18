// Old-style (non-generic) function typedefs.
typedef int Compare(int a, int b); /* expect: prefer-generic-function-type-aliases */
typedef void Callback(String msg); /* expect: prefer-generic-function-type-aliases */
typedef bool Predicate(Object o); /* expect: prefer-generic-function-type-aliases */
typedef String Formatter(int value); /* expect: prefer-generic-function-type-aliases */
typedef num Reducer(num a, num b); /* expect: prefer-generic-function-type-aliases */
typedef void Handler(); /* expect: prefer-generic-function-type-aliases */
