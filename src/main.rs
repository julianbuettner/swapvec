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
