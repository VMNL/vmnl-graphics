# Writing VMNL Issues and Pull Request Descriptions

Write in clear, natural English for a developer who has no access to the original conversation.
Start with the concrete problem or need and why it matters. Include enough context for the reader
to act on the document by itself. Use links for supporting details.

Prefer common words where they carry the same meaning. Keep the technical terms, API names, error
variants, conditions, and invariants that make the statement precise. Use short sentences with one
main idea each. Split a dense sentence before cutting useful information. Give each fact one place
and omit generic framing and recap paragraphs.

For example, write “Wait for the previous frame to finish before reusing its depth image” in place
of “Introduce a robust mechanism to ensure consistent resource lifecycle management.”

Base claims on the available source material. Distinguish current behavior, approved decisions,
proposals, planned checks, and observed results. State relevant gaps where the evidence ends.
Never invent technical details, implementation decisions, test runs, results, or measurements.
