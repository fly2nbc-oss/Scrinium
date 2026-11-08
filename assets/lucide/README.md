# Lucide icons

`lucide-subset.ttf` is a subset of `lucide-static@0.460.0/font/lucide.ttf`
(https://lucide.dev) with the icons that Scrinium uses. License: ISC, see `LICENSE`.

To create it again (codes from the package's `font/info.json`, see `theme::icon`):

```
pyftsubset lucide.ttf --unicodes=U+E0DC,U+E0D0,… --output-file=lucide-subset.ttf --no-hinting --layout-features=''
```
