# Dupe k*r*ill — a fast file deduplicator

Replaces files that have identical content with hardlinks, so that file data of all copies is stored only once, saving disk space. Useful for reducing sizes of multiple backups, messy collections of photos and music, countless copies of `node_modules`, macOS app bundles, and anything else that's usually immutable (since all hardlinked copies of a file will change when any one of them is changed).

## Features

* It's very fast and reasonably memory-efficient.
* Deduplicates incrementally as soon as duplicates are found.
* Replaces files atomically and it's safe to interrupt at any time.
* Proven to be reliable. Used for years without an issue.
* It's aware of existing hardlinks and supports merging of multiple groups of hardlinks.
* Gracefully handles symlinks and special files.

## Usage

[Download binaries from the releases page](https://github.com/kornelski/dupe-krill/releases).

Works on macOS and Linux. Windows is not supported.

If you have the [latest stable Rust](https://www.rust-lang.org/) (1.42+), build the program with either `cargo install dupe-krill` or clone this repo and `cargo build --release`.

```sh
dupe-krill -d <files or directories> # find dupes without doing anything
dupe-krill <files or directories> # find and replace with hardlinks
```

See `dupe-krill -h` for details.

### Output

Progress shows:

> `<number unique file bodies>`+`<number of hardlinks>` dupes. `<files checked>`+`<files skipped>` files scanned.

Symlinks, special device files, and 0-sized files are always skipped.

Don't try to parse program's usual output. Add `--json` option if you want machine-readable output. You can also use this program as a Rust library for seamless integration.

## How does hardlinking work?

Files are deduplicated by making a hardlink. They're not deleted. Instead, litreally the same file will exist in two or more directories at once. Unlike symlinks, the hardlinks behave like real files. Deleting one of hardlinks leaves other hardlinks unchanged. Editing a hardlinked file edits it in all places at once (except in some applications that delete & create a new file, instead of overwriting existing files). Hardlinking will make all duplicates of a file have the same file permissions.

This program will only deduplicate files larger than a single disk block (4KB, usually), because in many filesystems hardlinking tiny files may not actually save space. You can add `-s` flag to dedupe small files, too.

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
