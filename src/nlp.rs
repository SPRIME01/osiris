use anyhow::Result;
use ndarray::ArrayView1;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;
use tracing::warn;

const EMBEDDING_DIM: usize = 384;

pub struct Embedder {
    session: Session,
    tokenizer: Tokenizer,
}

impl Embedder {
    pub fn new() -> Result<Self> {
        let api = hf_hub::api::sync::Api::new()?;
        let repo = api.repo(hf_hub::Repo::new("sentence-transformers/all-MiniLM-L6-v2".to_string(), hf_hub::RepoType::Model));

        tracing::info!("Downloading/Loading ONNX model and tokenizer from HuggingFace...");
        let model_path = repo.get("onnx/model.onnx")?;
        let tokenizer_path = repo.get("tokenizer.json")?;

        let session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(1)?
            .commit_from_file(model_path)?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        Ok(Self { session, tokenizer })
    }

    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        if text.trim().is_empty() {
            warn!("Input text is empty. Returning zero vector.");
            return Ok(vec![0.0; EMBEDDING_DIM]);
        }

        let sentences: Vec<&str> = text.lines()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if sentences.is_empty() {
            warn!("No non-empty lines found. Returning zero vector.");
            return Ok(vec![0.0; EMBEDDING_DIM]);
        }

        let mut all_embeddings = Vec::new();

        for sentence in sentences {
            let encoding = self.tokenizer.encode(sentence, true)
                .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

            let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
            let attention_mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&x| x as i64).collect();
            let token_type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&x| x as i64).collect();

            let seq_len = input_ids.len();

            let attention_mask_array = ndarray::Array2::from_shape_vec((1, seq_len), attention_mask.clone())?;

            let input_ids_tensor = Tensor::from_array((vec![1, seq_len], input_ids))?;
            let attention_mask_tensor = Tensor::from_array((vec![1, seq_len], attention_mask))?;
            let token_type_ids_tensor = Tensor::from_array((vec![1, seq_len], token_type_ids))?;

            // Run inference
            let outputs = self.session.run(ort::inputs![
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor,
                "token_type_ids" => token_type_ids_tensor
            ])?;

            // The output tensor from all-MiniLM-L6-v2 is usually the first output (last_hidden_state)
            let last_hidden_state = outputs[0].try_extract_tensor::<f32>()?;
            let (_, data) = last_hidden_state;

            let mut sum_embedding = vec![0.0f32; EMBEDDING_DIM];
            let mut valid_tokens = 0.0f32;

            for i in 0..seq_len {
                if attention_mask_array[[0, i]] == 1 {
                    for j in 0..EMBEDDING_DIM {
                        // Data is flat, shaped [1, seq_len, 384]
                        sum_embedding[j] += data[i * EMBEDDING_DIM + j];
                    }
                    valid_tokens += 1.0;
                }
            }

            if valid_tokens > 0.0 {
                for j in 0..EMBEDDING_DIM {
                    sum_embedding[j] /= valid_tokens;
                }
            }

            let norm: f32 = sum_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for j in 0..EMBEDDING_DIM {
                    sum_embedding[j] /= norm;
                }
            }

            all_embeddings.push(sum_embedding);
        }

        let mut final_embedding = vec![0.0f32; EMBEDDING_DIM];
        let num_sentences = all_embeddings.len() as f32;

        for emb in all_embeddings {
            for j in 0..EMBEDDING_DIM {
                final_embedding[j] += emb[j];
            }
        }

        if num_sentences > 0.0 {
            for j in 0..EMBEDDING_DIM {
                final_embedding[j] /= num_sentences;
            }
        }

        Ok(final_embedding)
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let a_view = ArrayView1::from(a);
    let b_view = ArrayView1::from(b);

    let dot_product = a_view.dot(&b_view);
    let norm_a = a_view.dot(&a_view).sqrt();
    let norm_b = b_view.dot(&b_view).sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    let similarity = dot_product / (norm_a * norm_b);
    similarity.clamp(-1.0, 1.0)
}
