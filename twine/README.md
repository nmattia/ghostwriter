# Twine crate

This is a very hacky parser for [twine](https://twinery.org) archives.

The parser is very inefficient and avoids allocating any data by re-parsing and scanning the file whenever a passage is looked up.
