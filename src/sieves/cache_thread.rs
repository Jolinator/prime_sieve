use super::SieveConfig;
use super::optimized::{eratosthenes_algo_optimized, get_primes_from_bits};
use super::prime_files::dump_primes;
use std::cmp::min;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

pub(super) const EARLY_PERIOD: usize = 15015; // words: 3 * 5 * 7 * 11 * 13

//creates a mask that sieves primes 3 5 7 instantly
pub(super) fn build_early_pattern() -> [u64; EARLY_PERIOD] {
    let mut pattern = [!0u64; EARLY_PERIOD];
    for k in 0..EARLY_PERIOD * 64 {
        let n = 2 * k + 1;
        if n % 3 == 0 || n % 5 == 0 || n % 7 == 0 || n % 11 == 0 || n % 13 == 0 {
            pattern[k / 64] &= !(1u64 << (k % 64));
        }
    }
    pattern
}

fn init_with_early_pattern(nums: &mut [u64], pattern: &[u64; EARLY_PERIOD], word_offset: usize) {
    let mut idx = word_offset % EARLY_PERIOD;
    let mut rest = nums;
    while !rest.is_empty() {
        let n = rest.len().min(EARLY_PERIOD - idx);
        let (head, tail) = rest.split_at_mut(n);
        head.copy_from_slice(&pattern[idx..idx + n]);
        rest = tail;
        idx = 0;
    }
}

//Sieve with set primes in a range
fn sieve_with_primes(
    start_odd: usize,
    stop_odd: usize,
    primes: &[usize],
    pattern: &[u64; EARLY_PERIOD],
    nums: &mut [u64],
) -> Vec<usize> {
    let mut state = MarkState::default();
    mark_composites(
        start_odd, stop_odd, primes, pattern, nums, &mut state, true, false,
    );
    get_primes_from_bits(start_odd, stop_odd, nums)
}

// Sieving state carried between consecutive segments.
#[derive(Default)]
pub(super) struct MarkState {
    // next[k]: bit index of the next multiple of the k-th small prime
    next: Vec<usize>,
    // (prime, bit offset) of large primes, in a ring indexed by segments ahead of `head`
    buckets: Vec<Vec<(u32, u32)>>,
    head: usize,
}

fn first_bucketed(sieving: &[usize], stride: usize) -> usize {
    sieving.partition_point(|&p| p < stride)
}

fn first_hit(start_odd: usize, p: usize) -> usize {
    let mut counter = (start_odd / p) * p;
    if counter % 2 == 0 {
        counter += p;
    } else if counter < start_odd {
        counter += 2 * p;
    }
    (counter - start_odd) / 2
}

// `first` must be false only for the segment right after the one that produced `state`.
// With `bucket_large`, primes of at least one segment length use buckets.
pub(super) fn mark_composites(
    start_odd: usize,
    stop_odd: usize,
    primes: &[usize],
    pattern: &[u64; EARLY_PERIOD],
    nums: &mut [u64],
    state: &mut MarkState,
    first: bool,
    bucket_large: bool,
) {
    // 128 because each word is 64 bits and each bit is an odd number so 1 bit for every 2 integers
    let word_offset = (start_odd - 1) / 128;
    init_with_early_pattern(nums, pattern, word_offset);

    let sieving = &primes[6..];
    let stride = nums.len() * 64; // bits per full segment
    let bits = (stop_odd - start_odd).div_ceil(2);
    let split = if bucket_large {
        first_bucketed(sieving, stride)
    } else {
        sieving.len()
    };
    let (small, large) = sieving.split_at(split);

    if first {
        state.next.clear();
        state
            .next
            .extend(small.iter().map(|&p| first_hit(start_odd, p)));

        if let Some(&max_p) = large.last() {
            let ring = max_p / stride + 2;
            state.buckets.iter_mut().for_each(Vec::clear);
            state.buckets.resize_with(ring, Vec::new);
            state.head = 0;
            for &p in large {
                let idx = first_hit(start_odd, p);
                let slot = (idx / stride) % ring;
                state.buckets[slot].push((p as u32, (idx % stride) as u32));
            }
        }
    }

    let words = &mut nums[..bits.div_ceil(64)];
    for (&p, idx_slot) in small.iter().zip(state.next.iter_mut()) {
        let mut idx = *idx_slot;
        while idx < bits {
            debug_assert!(idx >> 6 < words.len());
            // SAFETY: idx < bits and words covers bits.div_ceil(64) words
            unsafe { *words.get_unchecked_mut(idx >> 6) &= !(1u64 << (idx & 63)) };
            idx += p; // 2p integers apart is p bits apart
        }
        *idx_slot = idx - stride;
    }

    if !large.is_empty() {
        let ring = state.buckets.len();
        let mut current = std::mem::take(&mut state.buckets[state.head]);
        for &(p, off) in &current {
            let off = off as usize;
            if off < bits {
                words[off >> 6] &= !(1u64 << (off & 63));
            }
            let n = off + p as usize;
            let slot = (state.head + n / stride) % ring;
            state.buckets[slot].push((p, (n % stride) as u32));
        }
        current.clear();
        state.buckets[state.head] = current;
        state.head = (state.head + 1) % ring;
    }
}

#[derive(Clone, Copy)]
pub(super) struct SieveJob<'a> {
    pub(super) primes: &'a [usize],
    pub(super) start_odds: &'a [usize],
    pub(super) stop_odds: &'a [usize],
    pub(super) pattern: &'a [u64; EARLY_PERIOD],
    pub(super) first_odd: usize,
    // When set, workers dump found primes to files in this directory instead of returning them.
    prime_dir: Option<&'a Path>,
    max_prime_vector_size: usize,
    pub(super) chunk_segments: usize,
}

// Primes up to this bound are enough to sieve every number up to `limit`: its square root,
// rounded up to an even number so the first number left to sieve is odd.
pub(super) fn base_limit(limit: usize) -> usize {
    ((limit as f64).sqrt() as usize | 1) + 1
}

// How the odd numbers between the base primes and `limit` are split into cache sized segments.
pub(super) struct SievePlan {
    first_odd: usize,
    start_odds: Vec<usize>,
    stop_odds: Vec<usize>,
    words_per_buffer: usize,
    thread_count: usize,
}

impl SievePlan {
    // `limit` must be large enough that its base primes leave at least one odd number to sieve.
    pub(super) fn new(limit: usize, config: &SieveConfig) -> Self {
        let first_odd = base_limit(limit) + 1;
        let first_aligned = (first_odd - 1) / 128 * 128 + 1;

        //round down to lower mult of 8 to fit in word
        let byte_split =
            ((config.cache_bytes as f64 * config.cache_usage_factor) as usize / 8 * 8).max(8);
        let bit_split = byte_split * 8;
        let odd_count = (limit - first_aligned) / 2 + 1;
        let workloads = odd_count.div_ceil(bit_split);

        let start_odds: Vec<usize> = (0..workloads)
            .map(|i| first_aligned + i * bit_split * 2)
            .collect();
        let stop_odds = start_odds
            .iter()
            .map(|&start| min(start + bit_split * 2, limit + 1))
            .collect();

        Self {
            first_odd,
            start_odds,
            stop_odds,
            words_per_buffer: bit_split / 64,
            thread_count: min(config.threads, workloads),
        }
    }

    pub(super) fn buffers(&self) -> Vec<Vec<u64>> {
        vec![vec![!0u64; self.words_per_buffer]; self.thread_count]
    }

    pub(super) fn job<'a>(
        &'a self,
        primes: &'a [usize],
        pattern: &'a [u64; EARLY_PERIOD],
        prime_dir: Option<&'a Path>,
        config: &SieveConfig,
    ) -> SieveJob<'a> {
        SieveJob {
            primes,
            start_odds: &self.start_odds,
            stop_odds: &self.stop_odds,
            pattern,
            first_odd: self.first_odd,
            prime_dir,
            max_prime_vector_size: config.max_prime_vector_size,
            chunk_segments: config.chunk_segments.max(1),
        }
    }
}

// Yields segment indexes until all are claimed; shared by every worker thread of a run.
pub(super) fn claim_segments(next: &AtomicUsize, total: usize) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        let i = next.fetch_add(1, Ordering::Relaxed);
        (i < total).then_some(i)
    })
}

// Runs `worker` on one thread per buffer and returns each thread's result.
pub(super) fn run_workers<T: Send>(
    job: SieveJob,
    buffers: &mut [Vec<u64>],
    worker: fn(SieveJob, &AtomicUsize, &mut [u64]) -> T,
) -> Vec<T> {
    let next_segment = AtomicUsize::new(0);

    thread::scope(|scope| {
        let handles: Vec<_> = buffers
            .iter_mut()
            .map(|nums| {
                let next_segment = &next_segment;
                scope.spawn(move || worker(job, next_segment, nums))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    })
}

type SegmentResult = (usize, Vec<usize>);

fn sieve_segment(job: SieveJob, i: usize, nums: &mut [u64]) -> Vec<usize> {
    let start = job.start_odds[i];
    let mut found = sieve_with_primes(start, job.stop_odds[i], job.primes, job.pattern, nums);
    if start < job.first_odd {
        found.retain(|&n| n >= job.first_odd);
    }
    found
}

// Returns the primes found when not storing, and how many were dumped to files when storing.
fn sieve_worker(
    job: SieveJob,
    next_segment: &AtomicUsize,
    nums: &mut [u64],
) -> (Vec<SegmentResult>, usize) {
    let mut results = Vec::new();
    let mut pending = Vec::new();
    let mut dumped = 0;
    for i in claim_segments(next_segment, job.start_odds.len()) {
        let found = sieve_segment(job, i, nums);
        match job.prime_dir {
            Some(dir) => {
                pending.extend(found);
                if pending.len() > job.max_prime_vector_size {
                    dumped += dump_primes(&mut pending, dir);
                }
            }
            None => results.push((i, found)),
        }
    }
    if let Some(dir) = job.prime_dir {
        dumped += dump_primes(&mut pending, dir);
    }
    (results, dumped)
}

static RUN_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub struct SieveOutput {
    pub count: usize,
    // Set when the primes were kept in memory (no storage directory was given).
    pub primes: Option<Vec<usize>>,
    // Set when the primes were dumped; the run's own directory.
    pub dir: Option<PathBuf>,
}

impl SieveOutput {
    fn in_memory(primes: Vec<usize>) -> Self {
        Self {
            count: primes.len(),
            primes: Some(primes),
            dir: None,
        }
    }

    fn stored(count: usize, dir: &Path) -> Self {
        Self {
            count,
            primes: None,
            dir: Some(dir.to_path_buf()),
        }
    }
}

// Sieves up to `limit`. With `store_prime_segments_path` the run gets its own directory inside it
// and every prime is dumped there (files named by first and last prime); only the count is
// returned. Without it, all primes are returned in memory.
pub fn segmented_sieve(
    limit: usize,
    config: &SieveConfig,
    store_prime_segments_path: Option<&Path>,
) -> SieveOutput {
    let run_dir = store_prime_segments_path.map(|path| {
        let id = RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = path.join(format!("run_{id}_limit_{limit}"));
        fs::create_dir_all(&dir).expect("could not create run primes directory");
        dir
    });
    sieve_run(limit, config, run_dir.as_deref())
}

pub fn sieve_in_memory(limit: usize, config: &SieveConfig) -> Vec<usize> {
    sieve_run(limit, config, None)
        .primes
        .expect("an in-memory run returns its primes")
}

fn sieve_run(limit: usize, config: &SieveConfig, run_dir: Option<&Path>) -> SieveOutput {
    if limit < 10000 {
        //sqroot of this number must have at least 4 primes (49). 62 is for safety.
        return store_or_return(eratosthenes_algo_optimized(limit + 1), run_dir);
    }

    //these are needed in memory as the sieving primes, so this inner run never stores
    let base_primes = sieve_in_memory(base_limit(limit), config);
    let plan = SievePlan::new(limit, config);
    let pattern = build_early_pattern();
    let job = plan.job(&base_primes, &pattern, run_dir, config);

    let (segments, dumped): (Vec<_>, Vec<_>) = run_workers(job, &mut plan.buffers(), sieve_worker)
        .into_iter()
        .unzip();
    let mut segments: Vec<SegmentResult> = segments.into_iter().flatten().collect();
    segments.sort_unstable_by_key(|&(i, _)| i);

    let mut all = base_primes;
    all.extend(segments.into_iter().flat_map(|(_, found)| found));
    let mut output = store_or_return(all, run_dir);
    output.count += dumped.into_iter().sum::<usize>();
    output
}

// Dumps the primes to `run_dir` if there is one, otherwise keeps them in memory.
fn store_or_return(mut primes: Vec<usize>, run_dir: Option<&Path>) -> SieveOutput {
    match run_dir {
        Some(dir) => SieveOutput::stored(dump_primes(&mut primes, dir), dir),
        None => SieveOutput::in_memory(primes),
    }
}
