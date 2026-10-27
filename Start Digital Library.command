#!/bin/bash
# Double-click in Finder to run Digital Library from source (keeps a Terminal window open; Ctrl+C stops it).
cd "$(dirname "$0")/library_app" || exit 1
exec python3 server.py
