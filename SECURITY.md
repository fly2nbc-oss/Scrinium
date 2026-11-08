# Security

## Supported versions

Only the latest release gets security fixes.

| Version | Supported |
|---|---|
| latest release | ✅ |
| older releases | ❌ |

## Reporting a vulnerability

Please do **not** open a public issue for security problems. Use GitHub's [private vulnerability reporting](https://github.com/fly2nbc-oss/Scrinium/security/advisories/new) instead.

Name the Scrinium version (*Settings › General › About Scrinium*), your operating system and the steps to reproduce the problem. Please do not attach real documents. You get a first answer within 14 days.

## What Scrinium does

- Scrinium works locally. It goes online only for: the update check at GitHub (only if allowed), the download of the AI model from Hugging Face, PDFium from GitHub and the language data for Tesseract (GitHub, fixed version; German, English, French, Spanish and Italian checked with SHA-256) – and, if set up, the external AI server, which then receives text and page image.
- Files are moved (or copied, if set) only after you confirm with *File*, and never overwritten (an existing name gets `_v2`, `_v3` …). Scrinium checks target folders from the AI, rules and your input: no `..`, no drives, only below the archive folder.
- When writing properties or saving rotated pages, signed and encrypted PDFs stay unchanged; other PDFs are only appended to (incremental update).
