use std::fmt::Write as _;
use std::fs;
use std::path::Path;

const LINE_WIDTH: usize = 80;

// Writes the primes to `primes_{start}_{stop}.txt` in `primes_dir`, comma separated.
fn save_primes(primes: &[usize], start: usize, stop: usize, primes_dir: &Path) {
    let path = primes_dir.join(format!("primes_{start}_{stop}.txt"));
    let mut out = String::with_capacity(primes.len() * 12);
    let mut line_start = 0;
    for (i, prime) in primes.iter().enumerate() {
        if i > 0 {
            // A line may run past the target width; the next prime then starts a new line.
            if out.len() - line_start >= LINE_WIDTH {
                out.push('\n');
                line_start = out.len();
            } else {
                out.push(' ');
            }
        }
        write!(out, "{prime}").unwrap();
        if i + 1 < primes.len() {
            out.push(',');
        }
    }
    out.push('\n');
    fs::write(path, out).expect("could not write primes file");
}

// Saves the primes using their first and last values as start and stop, then clears the vector.
// Returns how many primes were saved (0 and no file if the vector is empty).
pub fn dump_primes(primes: &mut Vec<usize>, primes_dir: &Path) -> usize {
    let (Some(&start), Some(&stop)) = (primes.first(), primes.last()) else {
        return 0;
    };
    save_primes(primes, start, stop, primes_dir);
    let saved = primes.len();
    primes.clear();
    saved
}
