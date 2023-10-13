from PyPDF2 import PdfReader
import sys
import json

filename = sys.argv[1]
reader = PdfReader(filename)
text = [page.extract_text() for page in reader.pages]
json.dump(text, open(filename + '.json', 'w'))

