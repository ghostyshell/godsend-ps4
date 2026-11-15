# Install games and homebrew

A PS4 installs **PKG** files. The app hands packages to GoldHEN's installer,
which pulls the file over HTTP and registers the title.

## From Downloads

1. Open Downloads. Files already in the download folder are listed.
2. Choose Send to PS4 on a `.pkg`.
3. The job starts, the console shows its install progress, and the title
   appears in Library when it finishes.

## From Homebrew

Homebrew, then Install on a card. The app downloads the release asset and
installs it the same way. Control4Free is the one the Controller page needs.

## From Convert

Pick a PKG, then Send to PS4. The same flow, for a file anywhere on disk.

## What happens on the wire

GoldHEN's installer takes a URL, not an upload. The app opens a one-shot HTTP
server on the interface that reaches the console, posts
`{"type":"direct","packages":["http://<your-ip>:<port>/game.pkg"]}`, and then
streams the file. Resume requests are answered with `206 Partial Content`.
Nothing is written to the console's storage twice.

If port 12800 does not answer, the app uploads the package to `/user/pkg`
instead and says so; start the installer on the console to finish.

## ZIPs

A `.zip` is unpacked locally first, then its contents are classified: a PKG
inside is installed, a payload is sent over the bin loader, and an unpacked
tree is reported as not installable (a PS4 installs packages, not folders).

## USB drives

A drive formatted FAT32 cannot hold a file above 4 GiB. Use Convert, Split
for FAT32 with the package selected, upload the parts, and merge them on the
computer before installing. exFAT drives take the file whole.
