// Type names must be UpperCamelCase.

class my_class {} /* expect: camel-case-types */

class Foo_Bar {} /* expect: camel-case-types */

enum my_enum { a, b } /* expect: camel-case-types */

mixin my_mixin {} /* expect: camel-case-types */

typedef my_callback = void Function(); /* expect: camel-case-types */

extension type my_id(int i) {} /* expect: camel-case-types */
