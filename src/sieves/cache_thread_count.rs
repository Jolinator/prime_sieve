use super::SieveConfig;
use super::cache_thread::{
    SieveJob, SievePlan, base_limit, build_early_pattern, claim_segments, mark_composites,
    run_workers, sieve_in_memory,
};
use super::optimized::eratosthenes_algo_optimized;
use std::sync::atomic::AtomicUsize;

//counts set bits in [lo, hi)
fn count_bits(nums: &[u64], lo: usize, hi: usize) -> usize {
    if lo >= hi {
        return 0;
    }
    let (first, last) = (lo / 64, (hi - 1) / 64);
    let mut total = 0;
    for w in first..=last {
        let mut bits = nums[w];
        if w == first {
            bits &= !0u64 << (lo % 64);
        }
        if w == last {
            let used = hi - w * 64;
            if used < 64 {
                bits &= (1u64 << used) - 1;
            }
        }
        total += bits.count_ones() as usize;
    }
    total
}

fn count_segment(job: SieveJob, i: usize, nums: &mut [u64]) -> usize {
    let start = job.start_odds[i];
    let stop = job.stop_odds[i];
    mark_composites(start, stop, job.primes, job.pattern, nums);
    let lo = job.first_odd.saturating_sub(start) / 2;
    let hi = (stop - start).div_ceil(2);
    count_bits(nums, lo, hi)
}

fn count_worker(job: SieveJob, next_segment: &AtomicUsize, nums: &mut [u64]) -> usize {
    claim_segments(next_segment, job.start_odds.len())
        .map(|i| count_segment(job, i, nums))
        .sum()
}

//same as segmented_sieve without storing, but only returns how many primes are <= limit
pub fn segmented_sieve_count(limit: usize, config: &SieveConfig) -> usize {
    if limit < 10000 {
        return eratosthenes_algo_optimized(limit + 1).len();
    }

    let base_primes = sieve_in_memory(base_limit(limit), config);
    let plan = SievePlan::new(limit, config);
    let pattern = build_early_pattern();
    let job = plan.job(&base_primes, &pattern, None, config);
    let counted: usize = run_workers(job, &mut plan.buffers(), count_worker)
        .into_iter()
        .sum();
    base_primes.len() + counted
}
