import sys
import os.path
from rust_json_parser import parse_json, parse_json_file, dumps

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python -m rust_json_parser <file_or_json_string>")
        sys.exit(1)

    arg = sys.argv[1]
    try:
        if os.path.exists(arg):
            result = parse_json_file(arg)
        else:
            result = parse_json(arg)
        print(dumps(result, indent=2))
    except ValueError as e:
        print(f"Parse error: {e}", file=sys.stderr)
        sys.exit(1)
    except IOError as e:
        print(f"File error: {e}", file=sys.stderr)
        sys.exit(1)