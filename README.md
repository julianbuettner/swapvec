# SwapVec

A vector which swaps to disk when exceeding a certain length.

Useful if creation and consumption of data should be
separated by time, but not much memory should be consumed.

## Features
- Multiplatform (Linux, Windows, MacOS)
- Creates temporary file only after exceeding threshold
- Works on `T: Serialize + Deserialize`
- Temporary file removed even when terminating the program
- Checksums to guarantee integrity

## Limitations
- Due to potentially doing IO, most actions are wrapped in a `Result`
- Currently, no "start swapping after n MiB" is implemented
  - Would need element wise space calculation due to heap elements (e.g. `String`)
- `Compression` currently does not compress. It is there to keep the API stable.
- No async support yet

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

### Extended Usage
This is the code for `cargo run` (`src/main.rs`).  
```rust
use swapvec::{SwapVec, SwapVecConfig};

const DATA_MB: u64 = 1024;

fn main() {
    let element_count = DATA_MB / 8;
    let big_iterator = 0..element_count * 1024 * 1024;

    let config = swapvec::SwapVecConfig {
        batch_size: 8 * 1024,
        ..SwapVecConfig::default()
    };
    let mut swapvec: SwapVec<_> = SwapVec::with_config(config);
    swapvec.consume(big_iterator.into_iter()).unwrap();

    println!("Data size: {}MB", DATA_MB);
    println!("Done. Batches written: {}", swapvec.batches_written());
    println!(
        "Filesize: {}MB",
        swapvec
            .file_size()
            .map(|x| x.unwrap() / 1024 / 1024)
            .unwrap_or(0)
    );
    println!("Read back");

    let read_back: Vec<_> = swapvec.into_iter().map(|x| x.unwrap()).collect();

    println!("{:#?}", read_back.len());
}
```

