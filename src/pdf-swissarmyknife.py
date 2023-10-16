from PyPDF2 import PdfReader
from alive_progress import alive_it
import sys
import json

filename = sys.argv[1]
reader = PdfReader(filename)
text = [page.extract_text() for page in alive_it(reader.pages)]
json.dump(text, open(filename + '.json', 'w'))

