# Conversions

Pick a file or folder on the Convert page and the page shows what it found:
kind, system, title id and size.

## Unpack a PKG

Writes the package's PFS tree to a folder: `eboot.bin`, `sce_sys`, `data`.
Useful to inspect a package, patch files, or check what a homebrew release
carries. Decryption is done in the app; no keys to paste.

## Split for FAT32

Writes `<name>.0`, `<name>.1`, ... at the chosen chunk size (4095 MiB by
default, safely under the 4 GiB limit). For USB drives formatted FAT32.

## Merge parts

Joins a split set back into one file. Pick the `.0` part and choose where the
merged file goes.

## Building a PKG

Out of scope: a fake PKG carries Sony's package signature, which this app does
not implement. Use a PKG builder on the computer, then install the result
here.
