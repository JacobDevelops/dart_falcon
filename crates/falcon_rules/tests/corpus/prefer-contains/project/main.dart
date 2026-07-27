import 'types.dart' show SelectedList;
import 'duplicate.dart' as duplicate;

bool selected(SelectedList values) => values.indexOf('x') >= 0; /* expect: prefer-contains */
bool unrelated(duplicate.SelectedList values) => values.indexOf('x') >= 0;
