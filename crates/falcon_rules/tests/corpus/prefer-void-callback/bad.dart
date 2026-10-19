void invoke(void Function() callback) => callback(); /* expect: prefer-void-callback */
void Function()? callback; /* expect: prefer-void-callback */
typedef Handler = void Function(); /* expect: prefer-void-callback */
