# Saves

Save data lives under `/user/home/<user>/savedata/<TITLE_ID>` on the console.
The Saves page lists every folder it finds, resolves the game title from the
installed library, and shows the file count and size.

## Back up

1. Tick one or more saves.
2. Choose Back up selected.

The job writes `ps4-saves-<timestamp>.zip` into the download folder. Each
entry keeps the title id as its top folder, so the zip can be restored by hand
if the app is gone.

## Restore

1. Choose Restore and pick a zip.
2. The job uploads every entry back to
   `/user/home/<first user>/savedata/<title id>`.

Close the game first: the console caches save files while a title is running.

## Delete

Delete asks for confirmation and removes the folder. There is no undo.

## What is not handled

- Saves from another account: restore writes to the console's first user.
- Resigning a save for a different account. Apollo Save Tool, on the Homebrew
  page, does that on the console.
