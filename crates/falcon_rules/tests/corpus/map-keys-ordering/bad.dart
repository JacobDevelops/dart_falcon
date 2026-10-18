final first = {'zebra': 1, 'apple': 2}; /* expect: map-keys-ordering */
final second = {"red": 1, "blue": 2}; /* expect: map-keys-ordering */
final raw = {r'zebra': 1, r'apple': 2}; /* expect: map-keys-ordering */
final mixedQuotes = {'zebra': 1, "apple": 2}; /* expect: map-keys-ordering */
final nested = {'outer': {'zebra': 1, 'apple': 2}}; /* expect: map-keys-ordering */
