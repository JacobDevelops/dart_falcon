final raw = r'https://example.org?to\u006ben=secret';
final fragment = 'https://example.org#ignored?' 'token=secret';
final path = 'https://example.org/to' 'ken=secret';
final values = Uri(queryParameters: {if (true) 'name': 'token', ...{'query': 'secret'}});
final dynamicSpread = Uri(queryParameters: {...values.queryParameters});
