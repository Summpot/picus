import os, re, sys

derive_re = re.compile(r'#\s*\[derive\s*\(([^)]*)\)\]')
struct_with_fields_re = re.compile(r'^\s*(pub\s+)?struct\s+(\w+)\s*\{')

results = []

for root, dirs, files in os.walk('crates'):
    for f in files:
        if not f.endswith('.rs'):
            continue
        path = os.path.join(root, f)
        with open(path, 'r', encoding='utf-8', errors='replace') as fp:
            try:
                content = fp.read()
                lines = content.split('\n')
            except:
                continue

        i = 0
        while i < len(lines):
            line = lines[i]
            m = struct_with_fields_re.match(line)
            if m:
                struct_name = m.group(2)
                derives = set()
                j = i - 1
                while j >= 0:
                    dj = lines[j].strip()
                    dm = derive_re.match(dj)
                    if dm:
                        for attr in dm.group(1).split(','):
                            derives.add(attr.strip())
                        j -= 1
                    elif dj == '' or dj.startswith('///') or dj.startswith('//') or dj.startswith('#[') or dj.startswith('#!['):
                        j -= 1
                    else:
                        break

                if 'Component' in derives and 'Resource' not in derives:
                    body_start = i + 1
                    brace_depth = 1
                    body_lines = []
                    for k in range(body_start, len(lines)):
                        body_lines.append(lines[k])
                        brace_depth += lines[k].count('{') - lines[k].count('}')
                        if brace_depth <= 0:
                            break
                    body = '\n'.join(body_lines)

                    enum_fields = re.findall(r':\s*(\w+)\s*[,}]', body)
                    option_enum_fields = re.findall(r':\s*Option<(\w+)>', body)
                    vec_enum_fields = re.findall(r':\s*Vec<(\w+)>', body)

                    all_candidates = set(enum_fields + option_enum_fields + vec_enum_fields)
                    # Exclude primitive/std types
                    primitives = {'String', 'bool', 'f32', 'f64', 'u8', 'u16', 'u32', 'u64',
                                  'i8', 'i16', 'i32', 'i64', 'usize', 'Entity', 'Vec2', 'Vec3',
                                  'Color', 'LinearRgba', 'UiRect', 'Val', 'Size', 'JustifyContent',
                                  'AlignItems', 'TextAlign', 'Overflow', 'StyleDef', 'CssProperty',
                                  'ZIndex', 'FontWeight'}
                    enum_candidates = all_candidates - primitives

                    if enum_candidates:
                        rel_path = os.path.normpath(path)
                        results.append((rel_path, i+1, struct_name, sorted(enum_candidates), body[:300]))
            i += 1

# Now verify each enum candidate is actually defined as an enum in the same file or nearby
print("=== Component structs with enum-typed fields (state markers) ===")
count = 0
for path, ln, name, enum_types, body in sorted(results):
    with open(path, 'r', encoding='utf-8', errors='replace') as fp:
        file_content = fp.read()

    local_enums = []
    for et in enum_types:
        if re.search(rf'(pub\s+)?enum\s+{re.escape(et)}\s*{{', file_content):
            local_enums.append(et)

    if local_enums:
        count += 1
        print(f"\n{path}:{ln}  pub struct {name} {{ ... }}")
        for et in local_enums:
            print(f"    enum field: {et}")
        # Show field lines
        field_lines = []
        for bl in body.split('\n'):
            bl_stripped = bl.strip()
            if bl_stripped and not bl_stripped.startswith('//') and not bl_stripped.startswith('#'):
                field_lines.append(bl_stripped)
        for fl in field_lines[:8]:
            print(f"      {fl}")

print(f"\n\nTotal Component structs with local enum fields: {count}")
