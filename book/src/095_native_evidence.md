# Native Evidence

This section groups the parts of LINC that compare header-side declarations against native
artifact-side evidence.

Read this section when you care about:

- what symbols an artifact exports or imports
- what native link inputs a package declares
- whether declarations and artifacts agree strongly enough for downstream use

The normal reading order inside this section is:

1. Symbol Inventories
2. Link Surface
3. Validation

Use this path when you are moving from "I parsed a header" to "I trust this native surface enough
to generate and link against it".
