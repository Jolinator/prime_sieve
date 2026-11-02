pub mod cache_thread;
pub mod cache_thread_count;
pub mod naive;
pub mod optimized;
pub mod prime_files;

#[derive(Clone, Copy)]
pub struct SieveConfig {
    pub cache_bytes: usize,
    pub threads: usize,
    pub cache_usage_factor: f64,
    // How many primes a worker holds before dumping them to a file.
    pub max_prime_vector_size: usize,
    // How many consecutive segments a worker claims at once (count mode).
    pub chunk_segments: usize,
}
