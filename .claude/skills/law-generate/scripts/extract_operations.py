#!/usr/bin/env python3
"""Extract operation definitions from schema.json and output markdown reference.

Usage:
    python extract_operations.py [schema_path]
    python extract_operations.py schema/v0.5.0/schema.json
    python extract_operations.py  # defaults to schema/latest/schema.json

Outputs a markdown document suitable for inclusion in reference.md, with:
- Operation table (name, required fields, optional fields)
- Per-operation YAML syntax examples
- Full machine_readable section structure
"""

import json
import sys
from pathlib import Path


def find_schema(schema_path=None):
    """Find schema file, searching common locations."""
    if schema_path:
        return Path(schema_path)
    # Search from cwd upward for schema/latest/schema.json
    cwd = Path.cwd()
    for parent in [cwd, *cwd.parents]:
        candidate = parent / "schema" / "latest" / "schema.json"
        if candidate.exists():
            return candidate
    raise FileNotFoundError("Could not find schema.json. Pass path as argument.")


def extract_operations(schema):
    """Extract operation definitions from schema definitions."""
    definitions = schema.get("definitions", {})
    operations = []

    for name, defn in sorted(definitions.items()):
        if not name.endswith("Operation"):
            continue

        props = defn.get("properties", {})
        required = set(defn.get("required", []))
        op_field = props.get("operation", {})

        # Get operation name(s) from const or enum
        if "const" in op_field:
            op_names = [op_field["const"]]
        elif "enum" in op_field:
            op_names = op_field["enum"]
        else:
            continue

        # Separate required/optional fields (exclude 'operation' and 'legal_basis')
        skip = {"operation", "legal_basis"}
        req_fields = sorted(k for k in required if k not in skip)
        opt_fields = sorted(
            k for k in props if k not in skip and k not in required
        )

        for op_name in op_names:
            operations.append({
                "name": op_name,
                "definition": name,
                "description": defn.get("description", ""),
                "required": req_fields,
                "optional": opt_fields,
                "properties": {
                    k: v for k, v in props.items() if k not in skip
                },
            })

    return operations


def extract_machine_readable_structure(schema):
    """Extract the machine_readable section structure."""
    definitions = schema.get("definitions", {})
    mr = definitions.get("machineReadableSection", {})
    return mr


def extract_regulatory_layers(schema):
    """Extract valid regulatory layer values."""
    props = schema.get("properties", {})
    layer = props.get("regulatory_layer", {})
    return layer.get("enum", [])


def format_field(name, prop):
    """Format a field description."""
    ref = prop.get("$ref", "")
    desc = prop.get("description", "")
    if ref:
        type_name = ref.split("/")[-1]
    elif "type" in prop:
        type_name = prop["type"]
    else:
        type_name = "any"
    return f"`{name}` ({type_name}): {desc}" if desc else f"`{name}` ({type_name})"


def render_markdown(operations, layers, schema_version):
    """Render operations as markdown reference."""
    lines = [
        f"# Law Generate - Technical Reference",
        f"",
        f"Based on schema {schema_version} (`schema/latest/schema.json`). Validate with `just validate`.",
        f"",
    ]

    # Regulatory layers
    lines.extend([
        "## Regulatory Layers",
        "",
        "Valid values for `regulatory_layer`:",
        "",
    ])
    for layer in layers:
        lines.append(f"- `{layer}`")
    lines.append("")

    # Operations summary table
    lines.extend([
        "## Operations Summary",
        "",
        "| Operation | Required Fields | Optional Fields |",
        "|-----------|----------------|-----------------|",
    ])
    for op in operations:
        req = ", ".join(f"`{f}`" for f in op["required"]) or "—"
        opt = ", ".join(f"`{f}`" for f in op["optional"]) or "—"
        lines.append(f"| `{op['name']}` | {req} | {opt} |")
    lines.append("")

    # Group by category
    categories = {
        "Arithmetic": ["ADD", "SUBTRACT", "MULTIPLY", "DIVIDE", "MIN", "MAX"],
        "Comparison": ["EQUALS", "GREATER_THAN", "LESS_THAN",
                       "GREATER_THAN_OR_EQUAL", "LESS_THAN_OR_EQUAL"],
        "Logical": ["AND", "OR", "NOT"],
        "Conditional": ["IF"],
        "Membership": ["IN"],
        "Collection": ["LIST"],
        "Date": ["AGE", "DATE_ADD", "DATE", "DAY_OF_WEEK", "DATE_DIFF",
                 "DATE_PART", "START_OF"],
        "Spatial": ["OVERLAPS"],
    }

    op_map = {op["name"]: op for op in operations}

    for category, op_names in categories.items():
        cat_ops = [op_map[n] for n in op_names if n in op_map]
        if not cat_ops:
            continue

        lines.extend([
            f"## {category} Operations",
            "",
        ])

        for op in cat_ops:
            lines.extend([
                f"### `{op['name']}`",
                "",
                op["description"] if op["description"] else "",
                "",
            ])

            # Required fields
            if op["required"]:
                lines.append("**Required:**")
                for f in op["required"]:
                    lines.append(f"- {format_field(f, op['properties'].get(f, {}))}")
                lines.append("")

            # Optional fields
            if op["optional"]:
                lines.append("**Optional:**")
                for f in op["optional"]:
                    lines.append(f"- {format_field(f, op['properties'].get(f, {}))}")
                lines.append("")

            # YAML example
            lines.append("```yaml")
            lines.append(f"operation: {op['name']}")
            for f in op["required"]:
                lines.append(f"{f}: $example_value")
            for f in op["optional"]:
                lines.append(f"# {f}: $optional_value")
            lines.append("```")
            lines.append("")

    # Uncategorized operations
    categorized = set()
    for ops in categories.values():
        categorized.update(ops)
    uncategorized = [op for op in operations if op["name"] not in categorized]
    if uncategorized:
        lines.extend(["## Other Operations", ""])
        for op in uncategorized:
            lines.extend([
                f"### `{op['name']}`",
                "",
                op.get("description", ""),
                "",
                "```yaml",
                f"operation: {op['name']}",
            ])
            for f in op["required"]:
                lines.append(f"{f}: $value")
            lines.append("```")
            lines.append("")

    return "\n".join(lines)


def main():
    schema_path = find_schema(sys.argv[1] if len(sys.argv) > 1 else None)
    print(f"Reading schema from: {schema_path}", file=sys.stderr)

    with open(schema_path) as f:
        schema = json.load(f)

    # Extract version from $schema URL or filename
    version = "v0.5.0"
    for part in str(schema_path).split("/"):
        if part.startswith("v") and "." in part:
            version = part
            break

    operations = extract_operations(schema)
    layers = extract_regulatory_layers(schema)

    print(f"Found {len(operations)} operations, {len(layers)} regulatory layers",
          file=sys.stderr)

    markdown = render_markdown(operations, layers, version)
    print(markdown)


if __name__ == "__main__":
    main()
