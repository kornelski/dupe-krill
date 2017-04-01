# Duplicate k*r*iller — a fast file deduplicator

Replaces duplicate files with identical content with hardlinks. Useful for reducing sizes of multiple backups, messy collections of photos and music, countless copies of `node_modules`, and anything else that's usually immutable (since all hardlinked copies of a file will change when any one of them is changed).

It's the first version of this program, so it may eat your data. It's been tested on macOS only. Use with caution (i.e. only stuff that is securely backed up).

## Features

* It's pretty fast and reasonably memory-efficient.
* Deduplicates incrementally as soon as duplicates are found.
* Replaces files atomically and it's safe to interrupt at any time.
* It's aware of hardlinks and supports merging of multiple groups of hardlinks.
* Gracefully handles symlinks and special files.

## Usage

If you have [Rust](https://www.rust-lang.org/), build the program with either `cargo install duplicate-kriller` or `cargo build --release`.

```sh
duplicate-kriller -d <files or directories> # find dupes without doing anything
duplicate-kriller <files or directories> # find and replace with hardlinks
```

See `duplicate-kriller -h` for details.

## The method

Theoretically, you could find all duplicate files by putting them in a giant hash table mapping their content to paths:

```rust
HashMap<Vec<u8>, Vec<Path>>
```

but of course that would use ludicrous amounts of memory. You can fix it by using hashes of the content instead of the content itself.

BTW, probability of an accidental hash collision here is about quadrillion quadrillions times less likely than a hard drive failure, and this program uses salted hashes to break intentional collisions.

```rust
HashMap<[u8; 16], Vec<Path>>
```

but that's still pretty slow, since you still need to read all the content of all files. You can save some work by comparing file sizes first:

```rust
HashMap<u64, HashMap<[u8; 20], Vec<Path>>
```

but it helps only a little, since file sizes are not uniformly distributed. You can eliminate a bit more of near-duplicates by comparing only beginnings of the files first:

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
