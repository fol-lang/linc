# LINC

LINC is the checked native-link and ABI-evidence boundary between PARC source
packages and downstream generators.

The canonical API is `linc::contract`. It accepts a complete PARC source
closure, preserves native-input and resolved-link order, and carries the
symbol, layout, callable-ABI, probe, policy, and diagnostic evidence needed to
validate that closure.

PARC owns source semantics, LINC owns schema-v2 link analysis, and generation
belongs downstream.

This crate establishes the LINC side of the H1 contract. It does not by itself
certify the full PARC -> LINC -> GERC pipeline or later hardening milestones.
