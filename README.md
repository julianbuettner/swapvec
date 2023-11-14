# SwapVec

A vector which swaps to disk when exceeding a certain length.

Useful if you do not want to use a queue, but first collecting
all data and then consuming it.

Imagine multiple threads slowly producing giant vectors of data,
passing it to a single consumer later on.

Or a CSV upload of multiple gigabytes to an HTTP server,
in which you want to validate every
line while uploading, without directly starting a Database
transaction or keeping everything in memory.

## Features
- Multiplatform (Linux, Windows, MacOS)
- Creates temporary file only after exceeding threshold
- Works on `T: Serialize + Deserialize + Clone`
- Temporary file removed even when terminating the program
- Checksums to guarantee integrity
- Can be moved across threads

## Limitations
- Due to potentially doing IO, most actions are wrapped in a `Result`
- Currently, no "start swapping after n MiB" is implemented
  - Would need element wise space calculation due to heap elements (e.g. `String`)
- `Compression` currently does not compress. It is there to keep the API stable.
- No async support (yet)
- When pushing elements or consuming iterators, SwapVec is "write only"
- Only forwards iterations
    - Can be reset though

## Examples

### Basic Usage

```rust
use swapvec::SwapVec;
let iterator = (0..9).into_iter();
let mut much_data = SwapVec::default();
// Starts using disk for big iterators
much_data.consume(iterator).unwrap();
for value in much_data.into_iter() {
    println!("Read back: {}", value.unwrap());
}
```

### Examples

Currently there is only one simple example,
doing some basic operations and getting metrics like
getting the batches/bytes written to file.
. Run it with

```bash
cargo run --example demo
```
