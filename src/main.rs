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
        "  options: --cache-usage-factor <f64> (0.5)  --max-prime-vector-size <n> (1000000)  --chunk-segments <n> (1024)"
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
// Falls back to `default` with a warning when the variable is missing or malformed.
fn env_min(name: &str, default: usize) -> usize {
    let parsed = env::var(name).ok().and_then(|value| {
        value
            .split(',')
            .map(|part| part.trim().parse::<usize>().ok())
            .collect::<Option<Vec<_>>>()
            .and_then(|parts| parts.into_iter().min())
    });
    parsed.unwrap_or_else(|| {
        eprintln!("warning: could not read {name} from the environment, using default {default}");
        default
    })
}

// Reads `--name value` or `--name=value`; exits with usage on a bad value.
fn flag_value<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> T {
    for (i, arg) in args.iter().enumerate() {
        let raw = if arg == name {
            args.get(i + 1).map(String::as_str)
        } else {
            arg.strip_prefix(name).and_then(|rest| rest.strip_prefix('='))
        };
        if arg == name || raw.is_some() {
            return raw.and_then(|v| v.parse().ok()).unwrap_or_else(|| {
                eprintln!("{name} needs a valid value");
                usage()
            });
        }
    }
    default
}

fn save_and_print(report: &str, differentiator: &str, results_dir: &Path) {
    let path = save_report(report, differentiator, results_dir);
    println!("Results saved to {}", path.display());
}

fn main() {
    //In my computer the fastest version of this algo found 4118054813 (up to 10^11) primes in ~10 seconds

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
        cache_bytes: env_min("CPU_L1D_CACHE_SIZE_PER_INSTANCE_KB", 32) * 1024,
        threads: env_min("CPU_LOGICAL_CORES", 12),
        cache_usage_factor: flag_value(&args, "--cache-usage-factor", 0.5),
        max_prime_vector_size: flag_value(&args, "--max-prime-vector-size", 1000000),
        chunk_segments: flag_value(&args, "--chunk-segments", 1024),
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
