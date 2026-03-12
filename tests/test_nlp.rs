use osiris::nlp;

#[test]
fn test_cosine_similarity_identical_vectors() {
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![1.0, 0.0, 0.0];
    assert_eq!(nlp::cosine_similarity(&a, &b), 1.0);
}

#[test]
fn test_cosine_similarity_orthogonal_vectors() {
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![0.0, 1.0, 0.0];
    assert_eq!(nlp::cosine_similarity(&a, &b), 0.0);
}

#[test]
fn test_cosine_similarity_opposite_vectors() {
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![-1.0, 0.0, 0.0];
    assert_eq!(nlp::cosine_similarity(&a, &b), -1.0);
}

#[test]
fn test_cosine_similarity_non_unit_vectors() {
    let a = vec![3.0, 0.0, 0.0];
    let b = vec![1.0, 0.0, 0.0];
    assert_eq!(nlp::cosine_similarity(&a, &b), 1.0);
}

#[test]
fn test_cosine_similarity_arbitrary_vectors() {
    let a = vec![1.0, 2.0, 3.0];
    let b = vec![4.0, 5.0, 6.0];

    // dot = 4 + 10 + 18 = 32
    // norm_a = sqrt(1 + 4 + 9) = sqrt(14)
    // norm_b = sqrt(16 + 25 + 36) = sqrt(77)
    // sim = 32 / (sqrt(14) * sqrt(77)) = 32 / sqrt(1078)
    let expected = 32.0 / (14.0f32.sqrt() * 77.0f32.sqrt());

    let result = nlp::cosine_similarity(&a, &b);
    assert!((result - expected).abs() < 1e-5);
}
