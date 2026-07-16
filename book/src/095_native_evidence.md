# Native Evidence

This section groups the parts of LINC that compare source-side intent against
native artifact-side evidence.

Read this section when you care about:

- what symbols an artifact exports or imports
- what native link inputs a package declares
- whether declarations and artifacts agree strongly enough for downstream use

The normal reading order inside this section is:

1. Symbol Inventories
2. Link Surface
3. Validation

Use this path when you are moving from "I have a source contract" to "I need
structured native observations for downstream policy". LINC does not by itself
certify that generation, linking, loading, or ABI use is safe.

The architectural rule stays the same here too:

- LINC consumes source-shaped input
- LINC emits evidence artifacts
- downstream generation still happens elsewhere
