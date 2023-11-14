use swapvec::{SwapVec, SwapVecConfig};

#[test]
fn reset_with_file() {
    let config = SwapVecConfig {
        compression: None,
        swap_after: 16,
        batch_size: 5,
    };

    let vector: Vec<u64> = (0..999).collect();

    let mut v = SwapVec::with_config(config);
    v.consume(vector.clone().into_iter()).unwrap();

    assert!(v.written_to_file());

    let mut iterator = v.into_iter();
    let vector_read_back: Vec<u64> = iterator.by_ref().map(|x| x.unwrap()).collect();
    assert_eq!(vector, vector_read_back);

    iterator.reset();
    let vector_read_back2: Vec<u64> = iterator.map(|x| x.unwrap()).collect();
    assert_eq!(vector, vector_read_back2);
}
