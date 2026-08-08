# LibAFL Documentation Book

This project contains the out-of-source `LibAFL--` documentation as a book.

Here you can find tutorials, examples, and detailed explanations.

For the API documentation instead, run `cargo doc` in the `LibAFlminusminus` root folder.

## Installation

To build the book, it is first needed to install `drawio-desktop`.
It is usually available in most ditributions.

Then, install `mdbook-drawio`, `lychee` and `mdbook`:
```bash
cargo install --git https://github.com/QBayLogic/mdbook-drawio
cargo install lychee
cargo install mdbook
```

## Usage

To build this book, you need [mdBook](https://github.com/rust-lang/mdBook).

`mdbook build` to build, `mdbook serve` to serve the book locally.

## Lints

The book uses `vale` to check for lints.
You will need to install it first.
