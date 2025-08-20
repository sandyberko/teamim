# TODO

## xilem app
- save image to file
- select multiple images and folders
- misses

## other

- use `ab_glyph` directly for better RTL support, warping, and finding bounding boxes
- Rect should be u32? right?
- boxedit: `box-container` isn't getting a size
- boxedit: unify `LSTMBox` and `CharBox` somehow
- `text2image` has an issue where it sometimes renders the last word of a page in a new line, and sometimes its box is written after the next page, which seems to confuse the training.
  - [`StringRenderer::RenderToImage`](https://github.com/tesseract-ocr/tesseract/blob/main/src/training/pango/stringrenderer.cpp#L725)
  - [`StringRenderer::ComputeClusterBoxes`](https://github.com/tesseract-ocr/tesseract/blob/main/src/training/pango/stringrenderer.cpp#L461)
  the boxes also seem to bw wrong?
  `training_Guttman Stam.box:87701` vs `bug_test_Guttman Stam.box:1835`

- train and check GROUND TRUTH
- recreate trainingdata, lstm?

- tesstrain make (line images!), langdata_ltsm
- is the leak reported by tesseract at shutdown real?
- sort: remove trailing spaces. check also in existing corrected_boxfiles
- sort: compact?
- sort+boxedit: use style instead of attrs
- boxedit: recognition doesn't work

### Tesstrain Make

I think I'll stick with box files:

- existing boxedit infrastructure
- unclear how to text2image
- unclear how to finetune with different chars
  However, still check for things like disambigs in `langdata_ltsm`, and plot

- 248/088 "ישלם המבער את הבער" is a mistake seemingly?
- "failed to read consonants" error
- IMPORTANT: qri-ktiv!
- diff not found 500
- check PSMs
- diff feature for training boxes
- more boxes?
- more augmentations:
  - variable inter-character spacing
  - wide letters: in training, maybe even in charset?

## Necessary

- detect font size of image?
- boxedit: recognize cached image

## Optimization

- sort worse recognitions and focus on them
- use `dissimilar` crate instead of `similar`
- embedded tessdata? `tess_from_mem` or something?
- Paseq glyph is empty?
- fix the whole `process-mam` thing
- process-mam: handle `lp-paseq` and such
- embed texts, cache other things
- TypeScript pedantic
- deduplicate `lib` and `main`
- fix the horrible inefficiencies of everything in `lib`. maybe just use some native UI?
- check placement of `Placement::After` te'amim, especially on lamed, nun and the like
- check what's up with zinor. is there a text that correctly uses zarqa instead?
- remove other _ta'amei emet_. see [Wikipedia](https://he.wikipedia.org/wiki/%D7%98%D7%A2%D7%9E%D7%99_%D7%94%D7%9E%D7%A7%D7%A8%D7%90)
- create_glyph to build script?
- optimize glyphs
  - save as Pix 1bit masks?
  - resize per set of images
- TryInto & TryFrom for BoundingBox
- box file origin bottom-left for training
- boxedit
  - diff
  - try non-input boxes again for overflow and more
  - blend mode for overlapping text
  - display keyboard resize direction
    - use corners for resize, so all arrows work
  - view/hide imgage etc. checkboxes
  - save name `.box`
- Training
  - explore training from scratch, as in that [Rashi project](https://gitlab.com/pninim.org/tessdata_heb_rashi/-/blob/main/tesseract_4.1.1/TRAINING.md)
- if Tesseract has text2image, surely it has API for fonts?
- Search
  - just find for perfect strings
  - trie? suffix tree? n-grams?
  - research from there

## Long term

- License, attribution (MAM, tesseract)
- errors in .ts and rust
