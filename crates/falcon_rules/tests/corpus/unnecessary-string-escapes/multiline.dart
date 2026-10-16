void examples() {
  print('''\
text''');
  print('''\ 
text''');
  print('''\\
text''');
  print(''' 	\ 
text''');
  print('''
text\q'''); /* expect: unnecessary-string-escapes */
}
