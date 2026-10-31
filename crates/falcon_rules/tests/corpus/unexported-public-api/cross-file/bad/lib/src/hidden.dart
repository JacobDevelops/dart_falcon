class Hidden {} /* expect: unexported-public-api, msg: "Public `Hidden` is used across the package but not exported from any public library." */
mixin HiddenMixin {} /* expect: unexported-public-api */
typedef HiddenAlias = Hidden; /* expect: unexported-public-api */
void hiddenFunction() {} /* expect: unexported-public-api */
final hiddenValue = 1; /* expect: unexported-public-api */
class Visible {}
