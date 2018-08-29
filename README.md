# Dupe k*r*ill — a fast file deduplicator

Replaces files that have identical content with hardlinks, so that file data of all copies is stored only once, saving disk space. Useful for reducing sizes of multiple backups, messy collections of photos and music, countless copies of `node_modules`, and anything else that's usually immutable (since all hardlinked copies of a file will change when any one of them is changed).


## Features

* It's pretty fast and reasonably memory-efficient.
* Deduplicates incrementally as soon as duplicates are found.
* Replaces files atomically and it's safe to interrupt at any time.
* It's aware of existing hardlinks and supports merging of multiple groups of hardlinks.
* Gracefully handles symlinks and special files.

## Usage

Works on macOS and Linux. Windows is not supported.

If you have [Rust](https://www.rust-lang.org/), build the program with either `cargo install dupe-krill` or `cargo build --release`.

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

## The method

Theoretically, you could find all duplicate files by putting them in a giant hash table aggregating file paths and using their content as the key:

```rust
HashMap<Vec<u8>, Vec<Path>>
```

but of course that would use ludicrous amounts of memory. You can fix it by using hashes of the content instead of the content itself.

BTW, probability of an accidental hash collision here is about quadrillion quadrillions times less likely than a hard drive failure, and this program uses salted hashes to break intentional collisions.

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
