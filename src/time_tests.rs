use std::{
    env,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::sieves::{
    SieveConfig,
    cache_thread::{segmented_sieve, sieve_in_memory},
    cache_thread_count::segmented_sieve_count,
    naive::eratosthenes_naive,
    optimized::eratosthenes_algo_optimized,
};

const COMPARE_LIMITS: [usize; 6] = [100, 1_000, 10_000, 1_000_000, 10_000_000, 100_000_000];
const COMPARE_CACHE_USAGE_FACTORS: [f64; 6] = [0.25, 0.5, 1.0, 1.5, 2.0, 5.0];
const COMPARE_MAX_PRIME_VECTOR_SIZES: [usize; 3] = [10_000, 100_000, 1_000_000];
const SCALING_LIMITS: [usize; 10] = [
    100,
    1_000,
    10_000,
    100_000,
    1_000_000,
    10_000_000,
    100_000_000,
    1_000_000_000,
    10_000_000_000,
    100_000_000_000,
];

fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let value = f();
    (value, start.elapsed())
}

fn timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before 1970")
        .as_nanos()
}

fn report_line(limit: usize, count: usize, config: &SieveConfig, elapsed: Duration) -> String {
    format!(
        "Limit: {limit}, Prime count: {count}, Cache bytes: {}, \
         Cache usage factor: {}, Thread count: {}, Time: {elapsed:?}\n",
        config.cache_bytes, config.cache_usage_factor, config.threads
    )
}

// Times every implementation at each limit and checks they all find the same primes.
pub fn test_compare(config: SieveConfig) -> String {
    let thread_counts = [1, config.threads / 2, config.threads];
    let mut report = String::new();

    for limit in COMPARE_LIMITS {
        let (naive, naive_time) = timed(|| eratosthenes_naive(limit));
        let (optimized, optimized_time) = timed(|| eratosthenes_algo_optimized(limit));
        let mut all_match = naive == optimized;
        writeln!(report, "Limit: {limit}, Same primes: {all_match}").unwrap();
        writeln!(report, "Naive Sieve time: {naive_time:?}").unwrap();
        writeln!(report, "Optimized Sieve time: {optimized_time:?}").unwrap();

        for cache_usage_factor in COMPARE_CACHE_USAGE_FACTORS {
            for max_prime_vector_size in COMPARE_MAX_PRIME_VECTOR_SIZES {
                for threads in thread_counts {
                    let variant = SieveConfig {
                        threads,
                        cache_usage_factor,
                        max_prime_vector_size,
                        ..config
                    };
                    let (primes, time) = timed(|| sieve_in_memory(limit, &variant));
                    writeln!(
                        report,
                        "Limit: {limit}, Cache Usage Factor: {cache_usage_factor}, \
                         Max Prime Vector: {max_prime_vector_size}, Thread Count: {threads}, \
                         Time: {time:?}"
                    )
                    .unwrap();
                    all_match &= primes == naive;
                }
            }
        }
        writeln!(report, "All implementations match: {all_match}\n").unwrap();
    }
    report
}

pub fn create_results_dir(use_cargo_path: bool) -> PathBuf {
    let dir = if use_cargo_path {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("results")
    } else {
        env::current_exe()
            .expect("could not determine executable path")
            .parent()
            .expect("executable path has no parent")
            .join("results")
    };
    fs::create_dir_all(&dir).expect("could not create results directory");
    dir
}

pub fn create_primes_dir(results_dir: &Path) -> PathBuf {
    let dir = results_dir.join(format!("primes_{}", timestamp_nanos()));
    fs::create_dir_all(&dir).expect("could not create primes directory");
    dir
}

pub fn save_report(report: &str, differentiator: &str, results_dir: &Path) -> PathBuf {
    let path = results_dir.join(format!(
        "result_test_{differentiator}_{}.txt",
        timestamp_nanos()
    ));
    fs::write(&path, report).expect("could not write results file");
    path
}

pub fn run_increasing_values(config: SieveConfig) -> String {
    SCALING_LIMITS
        .iter()
        .map(|&limit| run_count(limit, config))
        .collect()
}

// Sieves up to `limit`, storing every prime in a new run directory inside `primes_dir`.
// Returns the report and the run directory.
pub fn run_find_all(limit: usize, config: SieveConfig, primes_dir: &Path) -> (String, PathBuf) {
    let (output, elapsed) = timed(|| segmented_sieve(limit, &config, Some(primes_dir)));
    let run_dir = output.dir.expect("a stored run has a directory");
    (report_line(limit, output.count, &config, elapsed), run_dir)
}

pub fn run_count(limit: usize, config: SieveConfig) -> String {
    let (count, elapsed) = timed(|| segmented_sieve_count(limit, &config));
    report_line(limit, count, &config, elapsed)
}
