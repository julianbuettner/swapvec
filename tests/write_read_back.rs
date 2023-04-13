use swapvec::{SwapVec, SwapVecConfig};

#[test]
fn with_file() {
    let config = SwapVecConfig {
        compression: None,
        swap_after: 16,
        batch_size: 5,
    };

    let vector: Vec<u64> = (0..999).collect();

    let mut v = SwapVec::with_config(config);
    v.consume(vector.clone().into_iter()).unwrap();

    assert!(v.written_to_file());

    let vector_read_back: Vec<u64> = v.into_iter().map(|x| x.unwrap()).collect();

    assert_eq!(vector, vector_read_back);
}

#[test]
fn without_file() {
    let config = SwapVecConfig {
        compression: None,
        swap_after: 1001,
        batch_size: 5,
    };

    let vector: Vec<u64> = (0..999).collect();

    let mut v = SwapVec::with_config(config);
    v.consume(vector.clone().into_iter()).unwrap();

    assert!(!v.written_to_file());

    let vector_read_back: Vec<u64> = v.into_iter().map(|x| x.unwrap()).collect();

    assert_eq!(vector, vector_read_back);
}
