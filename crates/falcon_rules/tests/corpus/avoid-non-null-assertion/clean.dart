// A null assertion on a map index expression is explicitly allowed.
int readCount(Map<String, int> counts) => counts['key']!;
