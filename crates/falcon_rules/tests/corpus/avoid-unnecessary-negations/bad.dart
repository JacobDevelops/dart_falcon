bool trueLiteral() => !true; /* expect: avoid-unnecessary-negations */
bool falseLiteral() => !(false); /* expect: avoid-unnecessary-negations */
bool equal(bool flag, bool another) => !flag == !another; /* expect: avoid-unnecessary-negations */
bool unequal(bool flag, bool another) => (!flag) != (!another); /* expect: avoid-unnecessary-negations */
bool nested(bool a) => a && !true; /* expect: avoid-unnecessary-negations */
bool groupedEqual(bool a, bool b) => (!(a)) == (!(b)); /* expect: avoid-unnecessary-negations */
