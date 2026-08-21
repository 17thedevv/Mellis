# Mellis docs site

This starter uses a lightweight bilingual static layout built around the documentation already present in the repo.

## Structure

```text
docs/site/
├── index.html
├── assets/
│   └── styles.css
├── vi/
│   ├── index.html
│   ├── language.html
│   ├── compiler.html
│   └── docs.html
├── en/
│   ├── index.html
│   ├── language.html
│   ├── compiler.html
│   └── docs.html
└── README.md
```

## Local preview

```bash
cd docs/site
python -m http.server 8000
```

Then open:

- http://localhost:8000/vi/index.html
- http://localhost:8000/en/index.html

## Notes

- The site is centered on the documentation already in the project, instead of a public roadmap.
- Technical terms such as `MVIR`, `Trait`, `Lifetime`, `LLVM`, `Monomorphization`, and `Borrow Checker` stay in English for precision.
- This is intentionally a simple static approach until the project grows enough to justify a full Markdown/SSG migration.
