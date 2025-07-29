# Dupe k*r*ill — a fast file deduplicator

Replaces files that have identical content with hardlinks or reflinks (copy-on-write links), so that file data of all copies is stored only once, saving disk space. Useful for reducing sizes of multiple backups, messy collections of photos and music, countless copies of `node_modules`, macOS app bundles, and anything else that's usually immutable.

## Features

* It's very fast and reasonably memory-efficient.
* Deduplicates incrementally as soon as duplicates are found.
* Replaces files atomically and it's safe to interrupt at any time.
* Proven to be reliable. Used for years without an issue.
* It's aware of existing hardlinks and supports merging of multiple groups of hardlinks.
* **Supports both hardlinks and reflinks (copy-on-write)** for better compatibility and performance.
* Gracefully handles symlinks and special files.

## Usage

[Download binaries from the releases page](https://github.com/kornelski/dupe-krill/releases).

Works on macOS, Linux, and Windows (with ReFS filesystem for reflink support).

If you have the [latest stable Rust](https://www.rust-lang.org/) (1.42+), build the program with either `cargo install dupe-krill` or clone this repo and `cargo build --release`.

```sh
dupe-krill -d <files or directories> # find dupes without doing anything
dupe-krill <files or directories> # find and replace with hardlinks
dupe-krill --reflink <files or directories> # use reflinks (copy-on-write) when possible
dupe-krill --reflink-or-hardlink <files or directories> # try reflinks first, fallback to hardlinks
```

See `dupe-krill -h` for details.

### Output

It prints one duplicate per line. It prints *both* paths on the same line with the difference between them highlighted as `{first => second}`. 

Progress shows:

> `<number unique file bodies>`+`<number of hardlinks>` dupes. `<files checked>`+`<files skipped>` files scanned.

Symlinks, special device files, and 0-sized files are always skipped.

Don't try to parse program's usual output. Add `--json` option if you want machine-readable output. You can also use this program as a Rust library for seamless integration.

## How does deduplication work?

Files are deduplicated by making either a hardlink or a reflink, depending on the mode chosen:

### Hardlinks
The traditional approach creates hardlinks where literally the same file will exist in two or more directories at once. Unlike symlinks, hardlinks behave like real files. Deleting one hardlink leaves other hardlinks unchanged. Editing a hardlinked file edits it in all places at once (except in some applications that delete & create a new file instead of overwriting). Hardlinking will make all duplicates of a file have the same file permissions.

### Reflinks (Copy-on-Write)
A more modern approach that creates reflinks (copy-on-write links). Like hardlinks, reflinks initially point to the same data on disk, saving space. However, when one copy is modified, the filesystem automatically creates a separate copy of the modified portions only. This provides better isolation between files while still saving space for identical content.

**Platform Support:**
- **Linux**: Uses `FICLONE` ioctl (supported on Btrfs, XFS, and other modern filesystems)
- **macOS**: Uses `clonefile()` system call (supported on APFS)
- **Windows**: Uses `CopyFileEx` with `COPY_FILE_CLONE_FORCE` (requires Windows 10 v1903+ with ReFS filesystem)

This program will only deduplicate files larger than a single disk block (4KB, usually), because in many filesystems linking tiny files may not actually save space. You can add `-s` flag to dedupe small files, too.

### Nerding out about the fast deduplication algorithm

In short: it uses Rust's standard library `BTreeMap` for deduplication, but with a twist that allows it to compare files lazily, reading only as little file content as necessary.

----

Theoretically, you could find all duplicate files by putting them in a giant hash table aggregating file paths and using file content as the key:

```rust
HashMap<Vec<u8>, Vec<Path>>
```

but of course that would use ludicrous amounts of memory. You can fix it by using hashes of the content instead of the content itself.

> BTW, I can't stress enough how mind-bogglingly improbable accidental cryptographic hash collisions are. It's not just "you're probably safe if you're lucky". It's "creating this many files would take more energy than our civilisation has ever produced in all of its history".

```rust
HashMap<[u8; 16], Vec<Path>>
```

but that's still pretty slow, since you still read entire content of all the files. You can save some work by comparing file sizes first:

```rust
HashMap<u64, HashMap<[u8; 20], Vec<Path>>
```

but it helps only a little, since files with identical sizes are surprisingly common. You can eliminate a bit more of near-duplicates by comparing only beginnings of the files first:

```rust
HashMap<u64, HashMap<[u8; 20], HashMap<[u8; 20], Vec<Path>>>
```

and then maybe compare only the ends, and maybe a few more fragments in the middle, etc.:

```rust
HashMap<u64, HashMap<[u8; 20], HashMap<[u8; 20], HashMap<[u8; 20], Vec<Path>>>>
HashMap<u64, HashMap<[u8; 20], HashMap<[u8; 20], HashMap<[u8; 20], HashMap<[u8; 20], HashMap<[u8; 20], …>>>>
```

These endlessly nested hashmaps can be generalized. `BTreeMap` doesn't need to see the whole key at once. It only compares keys with each other, and the comparison can be done incrementally — by only reading enough of the file to show that its key is unique, without even knowing the full key.

```rust
BTreeMap<LazilyHashing<File>, Vec<Path>>
```

And that's what this program does (and a bit of wrangling with inodes).

The whole heavy lifting of deduplication is done by Rust's standard library `BTreeMap` and overloaded `<`/`>` operators that incrementally hash the files (yes, operator overloading that does file I/O is a brilliant idea. I couldn't use `<<`, unfortunately).
