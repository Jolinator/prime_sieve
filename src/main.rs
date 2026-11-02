mod sieves;
mod time_tests;

use std::{env, path::Path};

use sieves::SieveConfig;
use time_tests::{
    create_primes_dir, create_results_dir, run_count, run_find_all, run_increasing_values,
    save_report, test_compare,
};

fn usage() -> ! {
    eprintln!(
        "Usage: prime_sieve [compare | scaling (default) | count <n> | findall <n>] [--cargo]"
    );
    eprintln!(
        "  findall stores the primes; compare, scaling and count keep them in memory or only count them"
    );
    eprintln!(
        "  --cargo  save results in the project's results folder instead of beside the executable"
    );
    std::process::exit(1);
}

// Hybrid CPUs list one value per core type (e.g. "32,48"); use the smallest.
fn env_min(name: &str) -> usize {
    env::var(name)
        .unwrap_or_else(|_| panic!("{name} must be set"))
        .split(',')
        .map(|part| {
            part.trim()
                .parse::<usize>()
                .unwrap_or_else(|_| panic!("{name} must be a comma separated list of usize"))
        })
        .min()
        .expect("split always yields at least one part")
}

fn save_and_print(report: &str, differentiator: &str, results_dir: &Path) {
    let path = save_report(report, differentiator, results_dir);
    println!("Results saved to {}", path.display());
}

fn main() {
    //In my computer the fastest version of this algo found 4118054813 (up to 10^11) primes in 13.24 seconds

    let args: Vec<String> = env::args().skip(1).collect();
    let mode = match args.first() {
        Some(first) if !first.starts_with("--") => first.as_str(),
        _ => "scaling",
    };

    let limit = args.get(1).and_then(|value| value.parse::<usize>().ok());
    if matches!(mode, "count" | "findall") && limit.is_none() {
        eprintln!("{mode} requires a non-negative integer limit");
        usage();
    }

    let use_cargo_path = args.iter().any(|a| a == "--cargo");
    dotenvy::dotenv().ok();

    let config = SieveConfig {
        cache_bytes: env_min("CPU_L1D_CACHE_SIZE_PER_INSTANCE_KB") * 1024,
        threads: env_min("CPU_LOGICAL_CORES"),
        cache_usage_factor: 0.5,
        max_prime_vector_size: 1000000,
    };
    let results_dir = create_results_dir(use_cargo_path);

    match (mode, limit) {
        ("compare", _) => save_and_print(&test_compare(config), "test_compare", &results_dir),
        ("scaling", _) => save_and_print(
            &run_increasing_values(config),
            "scaling_count",
            &results_dir,
        ),
        ("count", Some(limit)) => save_and_print(
            &run_count(limit, config),
            &format!("count_{limit}"),
            &results_dir,
        ),
        ("findall", Some(limit)) => {
            let primes_dir = create_primes_dir(&results_dir);
            let (report, run_dir) = run_find_all(limit, config, &primes_dir);
            save_and_print(&report, &format!("findall_{limit}"), &results_dir);
            println!("Primes saved to {}", run_dir.display());
        }
        _ => usage(),
    }
}
