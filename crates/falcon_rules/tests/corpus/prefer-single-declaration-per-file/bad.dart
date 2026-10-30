class First {}
class Second {} /* expect: prefer-single-declaration-per-file */
mixin Third {} /* expect: prefer-single-declaration-per-file */
enum Fourth { value } /* expect: prefer-single-declaration-per-file */
extension Fifth on String {} /* expect: prefer-single-declaration-per-file */
extension type Sixth(int value) {} /* expect: prefer-single-declaration-per-file */
mixin class Seventh {} /* expect: prefer-single-declaration-per-file */
