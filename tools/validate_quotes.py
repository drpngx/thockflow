import argparse
import sys
import re

def validate_and_fix(input_path, output_path, fix):
    with open(input_path, 'r', encoding='utf-8') as f:
        content = f.read()

    errors = []
    fixed_content = content

    # Check for non-ASCII characters
    non_ascii_chars = [(i, c) for i, c in enumerate(content) if not c.isascii()]
    if non_ascii_chars:
        errors.append(f"Found {len(non_ascii_chars)} non-ASCII characters.")
        if fix:
            # Replacements
            replacements = {
                '\u2018': "'", # Left single quote
                '\u2019': "'", # Right single quote
                '\u201c': '"', # Left double quote
                '\u201d': '"', # Right double quote
                '\u2013': '-', # En dash
                '\u2014': '--', # Em dash
                '…': '...',   # Ellipsis
            }
            new_content = ""
            for char in fixed_content:
                if not char.isascii():
                    new_content += replacements.get(char, "") # Remove if unknown or use replacement
                else:
                    new_content += char
            fixed_content = new_content
    
    # Check for capitalization after sentence ending
    # Pattern: Period, whitespace(s), lowercase letter
    # We use a regex to find these.
    # Note: This is a simple heuristic.
    pattern = re.compile(r'\.(\s+)([a-z])')
    matches = list(pattern.finditer(fixed_content))
    
    if matches:
        errors.append(f"Found {len(matches)} sentences starting with lowercase letters.")
        if fix:
            # We need to apply fixes. Since strings are immutable, and indexes change if we modify, 
            # we can use a callback with sub
            def replace_func(match):
                return f".{match.group(1)}{match.group(2).upper()}"
            
            fixed_content = pattern.sub(replace_func, fixed_content)

    if errors:
        print("Validation errors found:")
        for error in errors:
            print(f"- {error}")
        
        if not fix:
            print("Run with --fix to automatically correct these errors.")
            sys.exit(1)
        else:
            print("Errors automatically fixed.")

    # Write to output
    if output_path:
        with open(output_path, 'w', encoding='utf-8') as f:
            f.write(fixed_content)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Validate quotes file.")
    parser.add_argument("--input", required=True, help="Input file path")
    parser.add_argument("--output", required=True, help="Output file path")
    parser.add_argument("--fix", action="store_true", help="Autofix errors")
    
    args = parser.parse_args()
    
    validate_and_fix(args.input, args.output, args.fix)
